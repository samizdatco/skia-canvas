use ash::vk::Handle;
use std::{sync::Arc, time::Duration};
use vulkano::{
    command_buffer::SubmitInfo,
    device::{
        Device, DeviceCreateInfo, DeviceExtensions, DeviceOwned, Queue, QueueCreateInfo
    },
    image::{Image, ImageMemory, ImageUsage},
    swapchain::{
        AcquireNextImageInfo, AcquiredImage, CompositeAlpha, PresentInfo, SemaphorePresentInfo, Surface, Swapchain,
        SwapchainCreateInfo, SwapchainPresentInfo
    },
    sync::{
        fence::{Fence, FenceCreateFlags, FenceCreateInfo},
        semaphore::Semaphore,
    },
    Validated, VulkanError, VulkanObject,
};
use skia_safe::{
    gpu::{self, backend_render_targets, backend_semaphores, surfaces, vk, FlushInfo, SemaphoresSubmitted},
    surface::BackendSurfaceAccess,
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
    store: DeviceStore, // rasters of pages drawn to this context
    backend: VulkanBackend, // <- MUST be last for proper drop ordering (after Frame and any other derived resources)
}

impl VulkanRenderer {
    pub fn for_window(_event_loop: &ActiveEventLoop, window: Arc<Window>, is_transparent: bool) -> Self {
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
                    image_extent: surface_capabilities.current_extent.unwrap_or(window.inner_size().into()),
                    image_usage: ImageUsage::COLOR_ATTACHMENT | (
                        // skia assumes wrapped render targets are also read/write
                        surface_capabilities.supported_usage_flags & (ImageUsage::TRANSFER_SRC | ImageUsage::TRANSFER_DST)
                    ),
                    min_image_count: surface_capabilities.min_image_count.max(2),
                    composite_alpha: surface_capabilities
                        .supported_composite_alpha
                        .into_iter()
                        .min_by_key(|mode| {
                            match mode {
                                CompositeAlpha::Opaque => if is_transparent { 3 } else { 0 },
                                CompositeAlpha::PreMultiplied => 1,
                                CompositeAlpha::Inherit => 2,
                                _ => 4,
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
        self.backend.resize(size);
    }

    pub fn draw(&mut self, page:Page, matrix:Matrix, props:SurfaceProps, matte:Color4f){
        let cache = Cache::Device(&self.store); // all the page rasters for *this* window's context
        let dpr = self.window.scale_factor() as f32;
        let plan = self.frame.begin(&page, &matrix, matte, dpr);

        let outcome = self.backend.render_frame(&self.window, &props, plan.take_snapshot,
            |canvas| self.frame.draw(canvas, &page, &matrix, matte, &plan, cache)
        );

        self.frame.commit(outcome, &page, matte, &plan); // potentially store the frame contents
        cache.sweep(); // release any rasters that have outlived their TTL
    }
}


struct SwapchainImage{
    image: Arc<Image>,        // the vulkan image for the next frame update
    rendered: Arc<Semaphore>, // signaled by skia's flush once drawing is complete/waited on by Present
    layout: vk::ImageLayout,  // UNDEFINED if the image is new, PRESENT_SRC_KHR once it's been flushed
}

impl SwapchainImage{
    fn new(image:Arc<Image>) -> Self{
        let rendered = Arc::new(Semaphore::from_pool(image.device().clone()).unwrap());
        Self{image, rendered, layout:vk::ImageLayout::UNDEFINED}
    }

    // wrap the image in a skia Surface (with its current layout) so it can be drawn into
    fn surface(&self, skia_ctx:&mut gpu::DirectContext, props:&SurfaceProps) -> skia_safe::Surface{
        let [width, height, _] = self.image.extent();
        let format = self.image.format();
        let (vk_format, color_type) = to_sk_format(&format)
            .unwrap_or_else(|| panic!("Vulkan: unsupported color format {:?}", format));

        let render_target = backend_render_targets::make_vk(
            (width.try_into().unwrap(), height.try_into().unwrap()),
            &unsafe {
                vk::ImageInfo::new(
                    self.image.handle().as_raw() as _,
                    vk::Alloc::default(),
                    vk::ImageTiling::OPTIMAL,
                    self.layout,
                    vk_format,
                    1,
                    None,
                    None,
                    None,
                    None,
                )
            },
        );

        surfaces::wrap_backend_render_target(
            skia_ctx,
            &render_target,
            gpu::SurfaceOrigin::TopLeft,
            color_type,
            None,
            Some(props),
        )
        .expect("Vulkan: Failed to wrap swapchain image in a skia surface")
    }

    // flush the surface's drawing and signal `rendered` when complete (recording its updated layout for next time)
    fn flush_for_present(&mut self, skia_ctx:&mut gpu::DirectContext, surface:&mut skia_safe::Surface){
        let mut signal = [unsafe{ backend_semaphores::make_vk(self.rendered.handle().as_raw() as _) }];
        let mut flush_info = FlushInfo::default();
        unsafe{ flush_info.set_signal_semaphores(&mut signal); }

        let signaled = skia_ctx.flush_surface_with_access(surface, BackendSurfaceAccess::Present, &flush_info);
        if signaled == SemaphoresSubmitted::No && skia_ctx.abandoned(){
            panic!("Vulkan: Skia couldn't flush to the swapchain image (device lost?)");
        }
        self.layout = vk::ImageLayout::PRESENT_SRC_KHR;
    }

    // queue the image for display, to be shown once `rendered` is signaled
    fn enqueue_present(&self, queue:&Arc<Queue>) -> bool{
        let ImageMemory::Swapchain{swapchain, image_index} = self.image.memory() else {
            unreachable!("Vulkan: SwapchainImage wraps an image the swapchain doesn't own")
        };
        let present_info = PresentInfo{
            wait_semaphores: vec![SemaphorePresentInfo::new(self.rendered.clone())],
            swapchain_infos: vec![SwapchainPresentInfo::swapchain_image_index(swapchain.clone(), *image_index)],
            ..Default::default()
        };
        let presented = queue.with(|mut q| unsafe{ q.present(&present_info) })
            .map_err(Validated::unwrap)
            .and_then(|mut results| results.next().unwrap());

        // return false if the swapchain needs to be recreated
        match presented{
            Ok(suboptimal) => !suboptimal, // the window resized/changed dpi/etc.
            Err(VulkanError::OutOfDate) => false, // the frame may or may not have been shown
            Err(e) => panic!("Vulkan: swapchain present failed: {e}"),
        }
    }
}

struct FramesInFlight{
    drawable: Vec<Arc<Semaphore>>, // signaled when the acquired image can be drawn to safely
    done: Vec<Arc<Fence>>,         // signaled once the GPU has rendered the image's drawing commands
    current: usize,                // points to the semaphore/fence slot to be used next
}

impl FramesInFlight{
    fn new(device:&Arc<Device>, count:usize) -> Self{
        let drawable = (0..count).map(|_|
            Arc::new(Semaphore::from_pool(device.clone()).unwrap())
        ).collect();
        let done = (0..count).map(|_|
            Arc::new(Fence::new(device.clone(), FenceCreateInfo{
                flags: FenceCreateFlags::SIGNALED, // initialize slots as being 'available'
                ..Default::default()
            }).unwrap())
        ).collect();
        Self{drawable, done, current:0}
    }

    // acquire the swapchain's next image (once the current slot is free) with a semaphore for when it becomes usable
    fn acquire_image(&self, swapchain:&Swapchain) -> Result<AcquiredImage, Validated<VulkanError>>{
        self.done[self.current].wait(None).expect("Vulkan: Failed to wait for frame fence");
        unsafe{
            swapchain.acquire_next_image(&AcquireNextImageInfo{
                semaphore: Some(self.drawable[self.current].clone()),
                ..Default::default()
            })
        }
    }

    // wait for the current slot's `drawable` semaphore to indicate it's no longer on screen
    fn wait_until_drawable(&self, surface:&mut skia_safe::Surface){
        let semaphore = unsafe{ backend_semaphores::make_vk(self.drawable[self.current].handle().as_raw() as _) };
        if !surface.wait(&[semaphore], false){
            panic!("Vulkan: Skia couldn't wait on the drawable semaphore (device lost?)");
        }
    }

    // leave the slot marked as in-use until its drawing commands are rendered, then advance to the next slot
    fn finish(&mut self, queue:&Arc<Queue>){
        let done = &self.done[self.current];
        unsafe{
            done.reset().expect("Vulkan: Failed to reset frame fence");
            queue.with(|mut q| q.submit(&[SubmitInfo::default()], Some(done)))
                .expect("Vulkan: Failed to submit frame fence");
        }
        self.current = (self.current + 1) % self.done.len();
    }
}

struct VulkanBackend{
    skia_ctx: gpu::DirectContext, // must be listed before parent queue to ensure proper drop order
    size: PhysicalSize<u32>,      // the size to (re)create the swapchain at
    swapchain: Arc<Swapchain>,    // vulkan's double-buffering coordinator
    images: Vec<SwapchainImage>,  // rebuilt along with the swapchain
    in_flight: FramesInFlight,    // per-frame sync state (acquire → draw → present)
    is_valid: bool,               // false once the swapchain needs to be recreated before the next frame
    queue: Arc<Queue>,
}

impl Drop for VulkanBackend{
    fn drop(&mut self) {
        self.queue.with(|mut q| q.wait_idle()).ok(); // let queued work complete before dropping semaphores/fences
        self.skia_ctx.release_resources_and_abandon();
    }
}

impl VulkanBackend{
    fn new(queue:Arc<Queue>, swapchain:Arc<Swapchain>) -> Self{
        // create a DirectContext that will let us use a surface & canvas to draw into swapchain images
        let device = queue.device();
        let skia_ctx = make_direct_context(device, &queue)
            .expect("Vulkan: Failed to create Skia direct context");

        let size = swapchain.image_extent().into();
        let in_flight = FramesInFlight::new(device, 2); // use 2 slots so CPU can record while GPU executes
        let is_valid = false; // mark invalid so the first render_frame() will set up the swapchain and images
        let images = vec![]; // start with an empty vec (to be populated along with the swapchain)

        Self{skia_ctx, swapchain, images, in_flight, size, is_valid, queue}
    }

    // record the new window size and recreate the swapchain to match
    fn resize(&mut self, size:PhysicalSize<u32>){
        // a `set_size` followed by its own `Resized` event reports the same size twice, so only recreate once
        if size == self.size && self.is_valid { return }

        self.size = size;
        self.is_valid = false;
        self.prepare_swapchain();
    }

    // recreate the swapchain (and the per-image state) if it's been flagged as invalid
    fn prepare_swapchain(&mut self){
        if self.is_valid { return }

        // usually the surface sets the swapchain's size, but on wayland it's the reverse (so use winit's size there)
        let surface_caps = self.swapchain.device().physical_device()
            .surface_capabilities(self.swapchain.surface(), Default::default())
            .expect("Vulkan: Failed to query surface capabilities");
        let extent = surface_caps.current_extent.unwrap_or_else(|| {
            let requested:[u32; 2] = self.size.into();
            std::array::from_fn(|i| requested[i].clamp(
                surface_caps.min_image_extent[i], surface_caps.max_image_extent[i])
            )
        });

        // recreate the swapchain at the new size (but ignore minimized windows that report a 0×0 extent)
        if extent[0] > 0 && extent[1] > 0 {
            // make sure all the old images have rendered/presented before dropping them
            self.queue.with(|mut q| q.wait_idle()).ok();

            let (new_swapchain, new_images) = self
                .swapchain
                .recreate(SwapchainCreateInfo {
                    image_extent: extent,
                    ..self.swapchain.create_info()
                })
                .expect("Vulkan: Failed to recreate swapchain");

            self.swapchain = new_swapchain;
            self.images = new_images.into_iter().map(SwapchainImage::new).collect();
            self.is_valid = true;
        }
    }

    // acquire an image, let the callback draw into it, then present it (potentially returning a snapshot if requested)
    fn render_frame<F>(&mut self, window:&Window, props:&SurfaceProps, take_snapshot:bool, f:F) -> RenderOutcome
        where F:FnOnce(&skia_safe::Canvas)
    {
        // recreate the swapchain first if a suboptimal/out-of-date result (or a zero-sized resize) left it invalid
        self.prepare_swapchain();

        // get the next image to draw into (blocking if the GPU is still working through the existing slots)
        let index = match self.in_flight.acquire_image(&self.swapchain).map_err(Validated::unwrap) {
            Ok(acquired) => {
                self.is_valid &= !acquired.is_suboptimal;
                acquired.image_index
            }
            Err(VulkanError::OutOfDate) => {
                // skip the frame and retry after the swapchain has been recreated
                self.is_valid = false;
                return RenderOutcome::Skipped;
            }
            Err(e) => panic!("Vulkan: Failed to acquire next image: {e}"),
        };

        // create a skia Surface that renders to the swapchain image
        let swapchain_image = &mut self.images[index as usize];
        let mut surface = swapchain_image.surface(&mut self.skia_ctx, props);

        // wait until the image is no longer on screen before letting skia draw to it
        self.in_flight.wait_until_drawable(&mut surface);
        f(surface.canvas());

        // snapshot the frame (if requested) so `Frame` can reuse it as the base for the next draw
        let image = take_snapshot.then(|| surface.image_snapshot_with_bounds(surface.image_info().bounds())).flatten();

        // flush the image's drawing commands then submit to the GPU
        swapchain_image.flush_for_present(&mut self.skia_ctx, &mut surface);
        self.skia_ctx.submit(gpu::SyncCpu::No);

        // let skia's GPU resource cache keep glyph atlases, textures, etc. warm across frames,
        // only purging resources that have gone unused for over a second
        self.skia_ctx.perform_deferred_cleanup(Duration::from_secs(1), None);

        // fence off this frame's work and advance to the next slot
        self.in_flight.finish(&self.queue);

        // tell winit a present is about to happen (used to schedule frame callbacks on Wayland)
        window.pre_present_notify();

        // queue the image for display (to be shown once the GPU finishes drawing) and recheck the swapchain's validity
        self.is_valid &= swapchain_image.enqueue_present(&self.queue);

        RenderOutcome::Rendered(image)
    }
}
