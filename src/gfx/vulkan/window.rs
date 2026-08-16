use ash::vk::Handle;
use std::{sync::Arc, time::Duration};
use vulkano::{
    device::{
        Device, DeviceCreateInfo, DeviceExtensions, DeviceOwned, Queue, QueueCreateInfo
    },
    image::{view::ImageView, ImageUsage},
    render_pass::{Framebuffer, FramebufferCreateInfo, RenderPass},
    swapchain::{
        acquire_next_image, CompositeAlpha, Surface, Swapchain, SwapchainAcquireFuture, SwapchainCreateInfo, SwapchainPresentInfo
    },
    sync::{self, GpuFuture},
    Validated, VulkanError, VulkanObject,
};
use skia_safe::{
    gpu::{self, backend_render_targets, surfaces, vk},
    Color4f, Matrix, SurfaceProps
};
use winit::{
    dpi::PhysicalSize,
    event_loop::ActiveEventLoop,
    window::Window,
};
use crate::gfx::page::Page;
use crate::gfx::RenderOutcome;
use crate::gfx::cache::{Cache, DeviceStore};
use crate::gfx::framebuffer::Frame;
use super::{VK_FORMATS, to_sk_format, VulkanShared, make_direct_context};

pub struct VulkanRenderer{
    window: Arc<Window>,
    frame: Frame, // framebuffer content from the last render (in case the next draws atop it)
    store: DeviceStore, // rasters of embedded canvases drawn to this context
    backend: VulkanBackend, // <- MUST be last for proper drop ordering (after Frame and any other derived resources)
}

impl VulkanRenderer {
    pub fn for_window(_event_loop: &ActiveEventLoop, window: Arc<Window>, _is_transparent: bool) -> Self {
        // all windows and the offscreen engine share one instance/physical-device; each window
        // keeps its own swapchain, logical device, queue, and Skia DirectContext
        let shared = VulkanShared::get().expect("Vulkan: Initialization failed");
        let instance = shared.instance.clone();

        // walk the ranked list of devices that *claim* they can present and choose the first one
        // that actually initializes (skipping over powered-down GPUs and the like)
        let surface = Surface::from_window(instance.clone(), window.clone()).unwrap();
        let (physical_device, device, queue) = shared.screen_devices(&surface)
            .find_map(|(physical_device, queue_family_index)| {
                let (device, mut queues) = Device::new(
                    physical_device.clone(),
                    DeviceCreateInfo {
                        enabled_extensions: DeviceExtensions {
                            khr_swapchain: true,
                            ..DeviceExtensions::empty()
                        },
                        queue_create_infos: vec![QueueCreateInfo {
                            queue_family_index,
                            ..Default::default()
                        }],
                        ..Default::default()
                    },
                ).ok()?;

                queues.next().map(|queue| (physical_device, device, queue))
            })
            .expect("Vulkan: no device can present to this window");

        // Create a swapchain to manage frame buffers and vsync
        let (swapchain, _images) = {
            // inspect the window to determine the type of framebuffer needed
            let surface_capabilities = physical_device
                .surface_capabilities(&surface, Default::default())
                .unwrap();

            // choose the first device format that is on the supported list
            let device_formats = physical_device
                .surface_formats(&surface, Default::default())
                .unwrap();
            let (image_format, _) = device_formats.clone()
                .into_iter()
                .find(|(fmt, _)| VK_FORMATS.contains(fmt))
                .unwrap_or_else(||
                    panic!(
                        "Vulkan: no format supported by Skia was found on device.\nSupported formats: {:?}\nDevice formats: {:?}",
                        VK_FORMATS,
                        device_formats
                    )
                );

            Swapchain::new(
                device.clone(),
                surface,
                SwapchainCreateInfo {
                    image_format,
                    image_extent: window.inner_size().into(),
                    image_usage: ImageUsage::COLOR_ATTACHMENT,
                    min_image_count: surface_capabilities.min_image_count.max(2),
                    composite_alpha: surface_capabilities
                        .supported_composite_alpha
                        .into_iter()
                        .min_by_key(|mode| {
                            // prefer transparency (TODO: this should be dependent on window background…)
                            match mode {
                                CompositeAlpha::PostMultiplied => 1,
                                CompositeAlpha::PreMultiplied => 2,
                                CompositeAlpha::Opaque => 3,
                                _ => 3,
                            }
                        })
                        .unwrap(),
                    ..Default::default()
                },
            )
            .unwrap()
        };

        Self{window, backend:VulkanBackend::new(queue, swapchain), frame:Frame::default(), store:DeviceStore::for_window()}
    }

    pub fn resize(&mut self, size: PhysicalSize<u32>) {
        self.frame.start_resizing();
        self.backend.swapchain_is_valid = false;
        self.backend.prepare_swapchain(size.into());
    }

    pub fn draw(&mut self, page:Page, matrix:Matrix, props:SurfaceProps, matte:Color4f){
        let cache = Cache::Device(&self.store); // all the embed rasters for *this* window's context
        let dpr = self.window.scale_factor() as f32;
        let plan = self.frame.begin(&page, &matrix, matte, dpr);

        let outcome = self.backend.render_frame(&self.window, &props, plan.take_snapshot,
            |canvas| self.frame.draw(canvas, &page, &matrix, matte, &plan, cache)
        );

        self.frame.commit(outcome, &page, matte, &plan); // potentially store the frame contents
        cache.sweep(); // release any rasters that have outlived their TTL
    }
}


struct VulkanBackend{
    framebuffers: Vec<Arc<Framebuffer>>,
    render_pass: Arc<RenderPass>,
    swapchain: Arc<Swapchain>,
    swapchain_is_valid: bool,
    last_render: Option<Box<dyn GpuFuture>>,
    skia_ctx: gpu::DirectContext, // must be listed before parent queue to ensure proper drop order
    queue: Arc<Queue>,
}

impl Drop for VulkanBackend{
    fn drop(&mut self) {
        self.skia_ctx.release_resources_and_abandon();
    }
}

impl VulkanBackend{
    fn new(queue:Arc<Queue>, swapchain:Arc<Swapchain>) -> Self{
        let device = queue.device();

        // Define the layout of the framebuffers and their role in the graphics pipeline
        let render_pass = vulkano::single_pass_renderpass!(
            device.clone(),
            attachments: {
                canvas_img: {
                    format: swapchain.image_format(),
                    samples: 1, // no need for MSAA since we're rendering 1:1
                    load_op: DontCare, // don't clear framebuffers ahead of time
                    store_op: DontCare, // we don't need the bitmap back after display
                },
            },
            pass: {
                // the only attachment will be the bitmap rendered by skia
                color: [canvas_img],
                depth_stencil: {},
            },
        )
        .unwrap();

        // Start with no framebuffers and flag that they need to be allocated before rendering
        let framebuffers = vec![];
        let swapchain_is_valid = false;

        // Hold onto the previous GpuFuture so we can wait on its completion before the next frame
        let last_render = Some(sync::now(device.clone()).boxed());

        // Create a DirectContext that will let us use a surface & canvas to draw into framebuffers
        let skia_ctx = make_direct_context(device, &queue)
            .expect("Vulkan: Failed to create Skia direct context");

        Self{queue, framebuffers, render_pass, swapchain, swapchain_is_valid, last_render, skia_ctx}
    }

    fn prepare_swapchain(&mut self, size: PhysicalSize<u32>) {
        // Only regenerate the swapchain/framebuffers if we've flagged that it's necessary
        if size.width > 0 && size.height > 0 && !self.swapchain_is_valid {
            let (new_swapchain, new_images) = self
                .swapchain
                .recreate(SwapchainCreateInfo {
                    image_extent: size.into(),
                    ..self.swapchain.create_info()
                })
                .expect("Vulkan: Failed to recreate swapchain");

            self.swapchain = new_swapchain;
            self.framebuffers = new_images
                .iter()
                .map(|image| {
                    Framebuffer::new(
                        self.render_pass.clone(),
                        FramebufferCreateInfo {
                            attachments: vec![ImageView::new_default(image.clone()).unwrap()],
                            ..Default::default()
                        },
                    )
                    .unwrap()
                })
                .collect();
            self.swapchain_is_valid = true;
        }
    }

    // outer Option is whether a frame was actually rendered, inner is the image
    fn render_frame<F>(&mut self, window:&Window, props:&SurfaceProps, take_snapshot:bool, f:F) -> RenderOutcome
        where F:FnOnce(&skia_safe::Canvas)
    {
        // make sure the framebuffers match the current window size
        self.prepare_swapchain(self.swapchain.image_extent().into());

        // no framebuffer available right now (swapchain out of date or suboptimal): skip this
        // frame and retry on the next redraw once the swapchain has been recreated
        let Some((image_index, acquire_future)) = self.get_next_frame() else {
            return RenderOutcome::Skipped;
        };

        // pull the appropriate framebuffer and create a skia Surface that renders to it
        let framebuffer = self.framebuffers[image_index as usize].clone();
        let mut surface = self.surface_for_framebuffer(framebuffer.clone(), props);

        // pass the suface's canvas to the user-provided callback
        f(surface.canvas());

        // save a copy of the frame bitmap for the cache (but only if requested)
        let image = take_snapshot.then(|| surface.image_snapshot_with_bounds(surface.image_info().bounds())).flatten();

        // display the result
        self.flush_framebuffer(window, image_index, acquire_future);

        RenderOutcome::Rendered(image)
    }

    fn get_next_frame(&mut self) -> Option<(u32, SwapchainAcquireFuture)> {
        // Request the next framebuffer and a GpuFuture for the render pass
        let (image_index, suboptimal, acquire_future) =
            match acquire_next_image(self.swapchain.clone(), None).map_err(Validated::unwrap) {
                Ok(r) => r,
                Err(VulkanError::OutOfDate) => {
                    self.swapchain_is_valid = false;
                    return None;
                }
                Err(e) => panic!("Vulkan: Failed to acquire next image: {e}"),
            };

        match suboptimal{
            // If the request was successful but suboptimal, schedule a swapchain recreation
            true => {
                self.swapchain_is_valid = false;
                None
            }
            // otherwise proceed with this frame
            false => Some((image_index, acquire_future))
        }
    }

    fn surface_for_framebuffer( &mut self, framebuffer: Arc<Framebuffer>, props: &SurfaceProps) -> skia_safe::Surface {
        let [width, height] = framebuffer.extent();
        let image_access = &framebuffer.attachments()[0];
        let image_object = image_access.image().handle().as_raw();

        let format = image_access.format();
        let (vk_format, color_type) = to_sk_format(&format)
            .unwrap_or_else(|| panic!("Vulkan: unsupported color format {:?}", format));

        let image_info = &unsafe {
            vk::ImageInfo::new(
                image_object as _,
                vk::Alloc::default(),
                vk::ImageTiling::OPTIMAL,
                vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                vk_format,
                1,
                None,
                None,
                None,
                None,
            )
        };

        let render_target = &backend_render_targets::make_vk(
            (width.try_into().unwrap(), height.try_into().unwrap()),
            image_info,
        );

        surfaces::wrap_backend_render_target(
            &mut self.skia_ctx,
            render_target,
            gpu::SurfaceOrigin::TopLeft,
            color_type,
            None,
            Some(props),
        )
        .unwrap()
    }

    fn flush_framebuffer(&mut self, window:&Window, image_index:u32, acquire_future:SwapchainAcquireFuture){
        // flush the canvas's contents to the framebuffer
        self.skia_ctx.flush_and_submit();

        // let the budgeted cache keep glyph atlases, images, etc. warm across frames,
        // only purging resources that have gone unused for over a second
        self.skia_ctx.perform_deferred_cleanup(Duration::from_secs(1), None);

        // reclaim leftover resources from the last frame
        self.last_render.as_mut().unwrap().cleanup_finished();

        // let winit know that rendering is complete
        window.pre_present_notify();

        // send the framebuffer to the gpu and display it on screen
        let future = self
            .last_render
            .take()
            .unwrap()
            .join(acquire_future)
            .then_swapchain_present(
                self.queue.clone(),
                SwapchainPresentInfo::swapchain_image_index(
                    self.swapchain.clone(),
                    image_index,
                ),
            )
            .then_signal_fence_and_flush();

        match future.map_err(Validated::unwrap) {
            Ok(future) => {
                self.last_render = Some(future.boxed());
            }
            Err(VulkanError::OutOfDate) => {
                let device = self.queue.device();
                self.last_render = Some(sync::now(device.clone()).boxed());
                self.swapchain_is_valid = false;
            }
            Err(e) => {
                panic!("Vulkan: swapchain flush failed: {e}");
            }
        };
    }

}
