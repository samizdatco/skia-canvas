// #![allow(unused_imports)]
use std::ptr;
use std::cell::RefCell;
use std::sync::{Arc, Mutex};

use skia_safe::gpu::{self, direct_contexts, surfaces, Budgeted, SurfaceOrigin};
use skia_safe::{ImageInfo, ISize, Surface, ColorSpace};

use skulpin_renderer::{CoordinateSystem, Renderer, RendererBuilder};
use skulpin_renderer::rafx::api::RafxExtents2D;
use vulkano::{
    VulkanLibrary, VulkanObject, Handle,
    device::{
        physical::{PhysicalDevice, PhysicalDeviceType}, Device, DeviceCreateInfo, 
        Queue, QueueCreateInfo, QueueFlags,
    },
    instance::{Instance, InstanceCreateFlags, InstanceCreateInfo},
};

#[cfg(feature = "window")]
use winit::{
    dpi::{LogicalSize, PhysicalSize},
    window::Window,
};

thread_local!(static VK_CONTEXT: RefCell<Option<VulkanEngine>> = RefCell::new(None));

pub struct VulkanEngine {
    context: gpu::DirectContext,
    _vk_graphics: VkGraphics,
}

impl VulkanEngine {
    fn init() {
        VK_CONTEXT.with(|cell| {
            let mut local_ctx = cell.borrow_mut();
            if local_ctx.is_none() {
                if let Ok(ctx) = VulkanEngine::new() {
                    local_ctx.replace(ctx);
                }
            }
        })
    }

    pub fn supported() -> bool {
        Self::init();
        VK_CONTEXT.with(|cell| cell.borrow().is_some()) && Self::surface(
            &ImageInfo::new_n32_premul(ISize::new(100, 100), Some(ColorSpace::new_srgb()))
        ).is_some()
    }

    fn new() -> Result<Self, String> {
        let vk_graphics = VkGraphics::new().unwrap();
        let context = {
            let get_proc = |of| unsafe {
                match vk_graphics.get_proc(of) {
                    Some(f) => f as _,
                    None => {
                        println!("resolve of {} failed", of.name().to_str().unwrap());
                        ptr::null()
                    }
                }
            };
            let backend_context = unsafe {
                gpu::vk::BackendContext::new(
                    vk_graphics.instance.handle().as_raw() as _,
                    vk_graphics.physical_device.handle().as_raw() as _,
                    vk_graphics.device.handle().as_raw() as _,
                    (
                        vk_graphics.queue.handle().as_raw() as _,
                        vk_graphics.queue.queue_index() as usize
                    ),
                    &get_proc,
                )
            };
            direct_contexts::make_vulkan(&backend_context, None)
        }.ok_or("Failed to create Vulkan context")?;

        Ok(Self {
            context,
            _vk_graphics: vk_graphics,
        })
    }

    pub fn surface(image_info: &ImageInfo) -> Option<Surface> {
        Self::init();
        VK_CONTEXT.with(|cell| {
            let local_ctx = cell.borrow();
            let mut context = local_ctx.as_ref().unwrap().context.clone();

            surfaces::render_target(
                &mut context,
                Budgeted::Yes,
                image_info,
                Some(4),
                SurfaceOrigin::BottomLeft,
                None,
                true,
                None,
            )
        })
    }
}


pub struct VulkanRenderer {
    skulpin: Arc<Mutex<Renderer>>,
}

unsafe impl Send for VulkanRenderer {}

#[cfg(feature = "window")]
impl VulkanRenderer {
    pub fn for_window(window: &Window) -> Self {
        let window_size = window.inner_size();
        let window_extents = RafxExtents2D {
            width: window_size.width,
            height: window_size.height,
        };

        let skulpin = RendererBuilder::new()
            .coordinate_system(CoordinateSystem::Logical)
            .build(&window, window_extents)
            .unwrap();
        let skulpin = Arc::new(Mutex::new(skulpin));
        VulkanRenderer { skulpin }
    }

    pub fn resize(&self, _size: PhysicalSize<u32>) {

    }

    pub fn draw<F: FnOnce(&skia_safe::Canvas, LogicalSize<f32>)>(
        &mut self,
        window: &Window,
        f: F,
    ) -> Result<(), String> {
        let size = window.inner_size();
        let window_extents = RafxExtents2D {
            width: size.width,
            height: size.height,
        };

        self.skulpin.lock().unwrap().draw(
            window_extents,
            window.scale_factor(),
            |canvas, coords| {
                let size = coords.window_logical_size();
                f(canvas, LogicalSize::new(size.width as f32, size.height as f32))
            }
        ).map_err(|e| format!("Rendering error {:?}", e))
    }
}




pub struct VkGraphics {
    pub library: Arc<VulkanLibrary>,
    pub instance: Arc<Instance>,
    pub physical_device: Arc<PhysicalDevice>,
    pub device: Arc<Device>,
    pub queue: Arc<Queue>,
}

impl VkGraphics{
    pub fn new() -> Result<VkGraphics, String> {
        let library = VulkanLibrary::new().unwrap();

        let instance = Instance::new(
            Arc::clone(&library),
            InstanceCreateInfo {
                flags: InstanceCreateFlags::ENUMERATE_PORTABILITY,
                ..Default::default()
            },
        )
        .expect("Vulkan: Could not create instance");
    
        let (physical_device, queue_family_index) = instance
            .enumerate_physical_devices()
            .unwrap()
            // No need for swapchain extension support.
            .filter_map(|p| {
                p.queue_family_properties()
                    .iter()
                    .position(|q| q.queue_flags.intersects(QueueFlags::GRAPHICS))
                    .map(|i| (p, i as u32))
            })
            .min_by_key(|(p, _)| match p.properties().device_type {
                PhysicalDeviceType::DiscreteGpu => 0,
                PhysicalDeviceType::IntegratedGpu => 1,
                PhysicalDeviceType::VirtualGpu => 2,
                PhysicalDeviceType::Cpu => 3,
                PhysicalDeviceType::Other => 4,
                _ => 5,
            })
            .expect("Vulkan: No suitable physical device found");
    
        println!(
            "Using device: {} (type: {:?}) on {:?}",
            physical_device.properties().device_name,
            physical_device.properties().device_type,
            std::thread::current().id()
        );
    
        let (device, mut queues) = Device::new(
            physical_device.clone(),
            DeviceCreateInfo {
                queue_create_infos: vec![QueueCreateInfo {
                    queue_family_index,
                    ..Default::default()
                }],
                ..Default::default()
            },
        )
        .expect("Vulkan: Failed to create device");
        
        let queue = queues.next().expect("Vulkan: Failed to create queue");

        Ok(Self{ library, instance, physical_device, device, queue })
    }

    pub unsafe fn get_proc(&self, of: gpu::vk::GetProcOf) -> Option<unsafe extern "system" fn()> {
        let fns = self.instance.fns();
        match of {
            gpu::vk::GetProcOf::Instance(instance, name) => {
                let vk_instance = ash::vk::Instance::from_raw(instance as _);
                self.library.get_instance_proc_addr(vk_instance, name)
            }
            gpu::vk::GetProcOf::Device(device, name) => {
                let vk_device = ash::vk::Device::from_raw(device as _);
                (fns.v1_0.get_device_proc_addr)(vk_device, name)
            }
        }
    }

}
