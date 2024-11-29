#![allow(dead_code)]
#![allow(unused_imports)]
use ash::vk::Handle;
use crossbeam::channel;
use std::{
    cell::RefCell, collections::HashMap, ptr, sync::Arc
};
use vulkano::{
    device::{
        physical::PhysicalDeviceType, Device, DeviceCreateInfo, DeviceExtensions, DeviceOwned, Queue, QueueCreateInfo, QueueFlags
    },
    image::{view::ImageView, ImageUsage},
    instance::{Instance, InstanceCreateFlags, InstanceCreateInfo},
    render_pass::{Framebuffer, FramebufferCreateInfo, RenderPass},
    swapchain::{
        self, acquire_next_image, CompositeAlpha, Surface, Swapchain, SwapchainAcquireFuture, SwapchainCreateInfo, SwapchainPresentInfo
    },
    sync::{self, GpuFuture},
    Validated, VulkanError, VulkanLibrary, VulkanObject,
};
use skia_safe::{
    gpu::{self, backend_render_targets, direct_contexts, surfaces, vk},
    Color, ColorType, Matrix, Paint,
};
use crate::context::page::Page;
use crate::gpu::RenderEvent;

use winit::{
    dpi::{LogicalSize, PhysicalSize},
    event_loop::ActiveEventLoop,
    window::{Window, WindowId},
};


pub struct VulkanRenderer{
    backend: channel::Sender<RenderEvent>,
}

impl VulkanRenderer {
    pub fn for_window(event_loop: &ActiveEventLoop, window: Arc<Window>) -> Self {
        let instance = {
            let library = VulkanLibrary::new().expect("Vulkan libraries not found on system");
            let required_extensions = Surface::required_extensions(event_loop);

            Instance::new(
                library,
                InstanceCreateInfo {
                    flags: InstanceCreateFlags::ENUMERATE_PORTABILITY, // support MoltenVK
                    enabled_extensions: required_extensions,
                    ..Default::default()
                },
            )
            .expect(&format!("Vulkan: could not create instance supporting: {:?}", required_extensions))
        };

        let device_extensions = DeviceExtensions {
            khr_swapchain: true, // we need a swapchain to manage repainting the window
            ..DeviceExtensions::empty()
        };

        let dpr = window.scale_factor();
        let surface = Surface::from_window(instance.clone(), window.clone()).unwrap();

        // Collect the list of available devices & queues then select ‘best’ one for our needs
        let (physical_device, queue_family_index) = instance
            .enumerate_physical_devices()
            .unwrap()
            .filter(|p| {
                // omit devices that don't support our swapchain requirement
                p.supported_extensions().contains(&device_extensions)
            })
            .filter_map(|p| {
                // for each device, find a graphics queue family that can handle our surface type
                // and filter out any devices that don't have one
                p.queue_family_properties()
                    .iter()
                    .enumerate()
                    .position(|(i, q)| {
                        q.queue_flags.intersects(QueueFlags::GRAPHICS)
                            && p.surface_support(i as u32, &surface).unwrap_or(false)
                        //  && p.presentation_support(_i as u32, event_loop).unwrap() // unreleased
                    })
                    .map(|i| (p, i as u32))
            })
            .min_by_key(|(p, _)| {
                // Sort the list of acceptible devices/queues to try to find the fastest
                match p.properties().device_type {
                    PhysicalDeviceType::DiscreteGpu => 0,
                    PhysicalDeviceType::IntegratedGpu => 1,
                    PhysicalDeviceType::VirtualGpu => 2,
                    PhysicalDeviceType::Cpu => 3,
                    PhysicalDeviceType::Other => 4,
                    _ => 5,
                }
            })
            .expect("Vulkan: no suitable physical device found");

        // Use the physical device we selected to initialize a device with a single queue
        let (device, mut queues) = Device::new(
            physical_device.clone(),
            DeviceCreateInfo {
                enabled_extensions: device_extensions,
                queue_create_infos: vec![QueueCreateInfo {
                    queue_family_index,
                    ..Default::default()
                }],
                ..Default::default()
            },
        )
        .expect("Vulkan: device initialization failed");

        let queue = queues.next().unwrap();

        // Create a swapchain to manage frame buffers and vsync
        let (swapchain, _images) = {
            // inspect the window to determine the type of framebuffer needed
            let surface = Surface::from_window(instance.clone(), window.clone()).unwrap();
            let surface_capabilities = physical_device
                .surface_capabilities(&surface, Default::default())
                .unwrap();
            let (image_format, _) = physical_device
                .surface_formats(&surface, Default::default())
                .unwrap()[0];

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

        let (tx, rx) = channel::unbounded::<RenderEvent>();
        std::thread::spawn(move || {
            let mut backend = VulkanBackend::new(queue, swapchain);
            while let Ok(event) = rx.recv() {
                if !rx.is_empty(){ continue } // drop all but the last frame in the queue

                match event{
                    RenderEvent::Resize(width, height) => {
                        backend.swapchain_is_valid = false;
                        backend.prepare_swapchain((width, height).into());
                    },
                    RenderEvent::Draw(page, matrix, matte) => {
                        let paint = Paint::default();
                        let (clip, _) = matrix.map_rect(page.bounds);
                        backend.render_frame(|canvas|{
                            canvas.reset_matrix();
                            canvas.scale((dpr as f32, dpr as f32));
                            canvas.clip_rect(clip, None, Some(true));
                            canvas.draw_picture(page.get_picture(matte).unwrap(), Some(&matrix), Some(&paint));
                        }).unwrap();
                    }
                }
            }
        });

        Self{backend:tx}
    }

    pub fn resize(&self, size: PhysicalSize<u32>) {
        self.backend.send( RenderEvent::Resize(size.width, size.height) ).ok();
    }

    pub fn render(&self, page:Page, matrix:Matrix, matte:Option<Color>){
        self.backend.send( RenderEvent::Draw(page, matrix, matte) ).ok();
    }
}


struct VulkanBackend{
    queue: Arc<Queue>,
    framebuffers: Vec<Arc<Framebuffer>>,
    render_pass: Arc<RenderPass>,
    swapchain: Arc<Swapchain>,
    swapchain_is_valid: bool,
    last_render: Option<Box<dyn GpuFuture>>,
    skia_ctx: gpu::DirectContext,
}

impl Drop for VulkanBackend{
    fn drop(&mut self) {
        self.skia_ctx.abandon();
    }
}

impl VulkanBackend{
    fn new(queue:Arc<Queue>, swapchain:Arc<Swapchain>) -> Self{
        let device = queue.device();
        let instance = device.instance();
        let library = instance.library();

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
        let skia_ctx = unsafe {
            let get_proc = |gpo| {
                let get_device_proc_addr = instance.fns().v1_0.get_device_proc_addr;

                match gpo {
                    vk::GetProcOf::Instance(instance, name) => {
                        let vk_instance = ash::vk::Instance::from_raw(instance as _);
                        library.get_instance_proc_addr(vk_instance, name)
                    }
                    vk::GetProcOf::Device(device, name) => {
                        let vk_device = ash::vk::Device::from_raw(device as _);
                        get_device_proc_addr(vk_device, name)
                    }
                }
                .map(|f| f as _)
                .unwrap_or_else(|| {
                    println!("Vulkan: failed to resolve {}", gpo.name().to_str().unwrap());
                    ptr::null()
                })
            };

            let direct_context = direct_contexts::make_vulkan(
                &vk::BackendContext::new(
                    instance.handle().as_raw() as _,
                    device.physical_device().handle().as_raw() as _,
                    device.handle().as_raw() as _,
                    (
                        queue.handle().as_raw() as _,
                        queue.queue_family_index() as usize,
                    ),
                    &get_proc,
                ),
                None,
            )
            .expect("Vulkan: Failed to create Skia direct context");

            direct_context
        };

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
                .expect("failed to recreate swapchain");

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

    fn render_frame<F>(&mut self, f:F) -> Result<(), String>
        where F:FnOnce(&skia_safe::Canvas)
    {
        // make sure the framebuffers match the current window size
        self.prepare_swapchain(self.swapchain.image_extent().into());

        if let Some((image_index, acquire_future)) = self.get_next_frame() {
            // pull the appropriate framebuffer and create a skia Surface that renders to it
            let framebuffer = self.framebuffers[image_index as usize].clone();
            let mut surface = self.surface_for_framebuffer(framebuffer.clone());

            // pass the suface's canvas and dimensions to the user-provided callback
            f(surface.canvas());

            // display the result
            self.flush_framebuffer(image_index, acquire_future);
        }

        Ok(())
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
                Err(e) => panic!("failed to acquire next image: {e}"),
            };

        // If the request was successful but suboptimal, schedule a swapchain recreation
        if suboptimal {
            self.swapchain_is_valid = false;
        }

        Some((image_index, acquire_future))
    }

    fn surface_for_framebuffer(
        &mut self,
        framebuffer: Arc<Framebuffer>,
    ) -> skia_safe::Surface {
        let [width, height] = framebuffer.extent();
        let image_access = &framebuffer.attachments()[0];
        let image_object = image_access.image().handle().as_raw();

        let format = image_access.format();
        let (vk_format, color_type) = match format {
            vulkano::format::Format::B8G8R8A8_UNORM => (
                skia_safe::gpu::vk::Format::B8G8R8A8_UNORM,
                ColorType::BGRA8888,
            ),
            _ => panic!("Vulkan: unsupported color format {:?}", format),
        };

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
            None,
        )
        .unwrap()
    }

    fn flush_framebuffer(&mut self, image_index:u32, acquire_future:SwapchainAcquireFuture){
        // flush the canvas's contents to the framebuffer
        self.skia_ctx.flush_and_submit();
        self.skia_ctx.free_gpu_resources();

        // reclaim leftover resources from the last frame
        self.last_render.as_mut().unwrap().cleanup_finished();

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
