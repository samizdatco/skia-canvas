#![allow(unused_imports)]
use std::{cell::RefCell, sync::{Arc, OnceLock}, time::{Instant, Duration}, ptr};
use serde_json::{json, Value};

use vulkano::{
    device::{
        physical::{PhysicalDevice, PhysicalDeviceType},
        Device, DeviceCreateInfo, Queue, QueueCreateInfo, QueueFlags,
    },
    instance::{Instance, InstanceCreateFlags, InstanceCreateInfo},
    Handle, VulkanLibrary, VulkanObject,
};

use skia_safe::gpu::vk::{BackendContext, GetProcOf};
use skia_safe::gpu::{direct_contexts, surfaces, Budgeted, DirectContext, SurfaceOrigin};
use skia_safe::{ColorSpace, ISize, ImageInfo, Surface};


thread_local!( static VK_CONTEXT: RefCell<Option<VulkanContext>> = const { RefCell::new(None) }; );
static VK_STATUS: OnceLock<Value> = OnceLock::new();
static VK_CONTEXT_LIFESPAN:Duration = Duration::from_secs(5);

#[derive(Debug)]
#[allow(dead_code)]
pub struct VulkanEngine {
    context: DirectContext,
    library: Arc<VulkanLibrary>,
    instance: Arc<Instance>,
    physical_device: Arc<PhysicalDevice>,
    device: Arc<Device>,
    queue: Arc<Queue>,
    last_use: Instant,
}

impl VulkanEngine {
    pub fn supported() -> bool {
        Self::status()["renderer"] == "GPU"
    }

    pub fn status() -> Value {
        VK_STATUS.get_or_init(||{
            // test whether a context can be created and do some one-time init if so
            let context = VulkanContext::new()
                .and_then(|mut ctx| match ctx.works(){
                    true => Ok(ctx),
                    false => Err("Vulkan device was instantiated but unable to render".to_string())
                });
                
            match context {
                Ok(context) => {
                    Self::spawn_idle_watcher(); // watch for inactive contexts and deallocate them

                    let device_props = context.physical_device.properties();
                    let (mode, gpu_type) = match device_props.device_type {
                        PhysicalDeviceType::IntegratedGpu => ("GPU", Some("Integrated GPU")),
                        PhysicalDeviceType::DiscreteGpu => ("GPU", Some("Discrete GPU")),
                        PhysicalDeviceType::VirtualGpu => ("GPU", Some("Virtual GPU")),
                        _ => ("CPU", Some("Software Rasterizer"))
                    };
                    json!({
                        "renderer": mode,
                        "device": gpu_type.map(|t| format!("{} ({})",
                            t, device_props.device_name)
                        ),
                        "driver":format!("{} ({})",
                            device_props.driver_id.map(|id| format!("{:?}", id) ).unwrap_or("Unknown Driver".to_string()),
                            device_props.driver_info.as_ref().unwrap_or(&"Unknown Version".to_string()),
                        ),
                        "api": "Vulkan",
                    })
                },
                Err(msg) => json!({
                    "renderer": "CPU",
                    "api": "Vulkan",
                    "device": "CPU-based renderer (Fallback)",
                    "driver": "N/A",
                    "error": msg,
                })    
            }
        }).clone()
    }

    fn spawn_idle_watcher(){
        rayon::spawn(move || loop{
            std::thread::sleep(Duration::from_secs(1));
            rayon::spawn_broadcast(|_|{
                // drop contexts that haven't been used in a while to free resources
                VK_CONTEXT.with_borrow_mut(|cell| {
                    cell.take_if(|engine|{
                        engine.last_use.elapsed() > VK_CONTEXT_LIFESPAN
                    });
                });
            });
        })
    }

    pub fn surface(image_info: &ImageInfo) -> Option<Surface> {
        match Self::supported() {
            false => None,
            true => VK_CONTEXT.with_borrow_mut(|local_ctx| {
                // lazily initialize this thread's context then create the surface with it
                local_ctx
                    .take()
                    .or_else(|| VulkanContext::new().ok() )
                    .and_then(|ctx|{
                        let ctx = local_ctx.insert(ctx);
                        ctx.surface(image_info)
                    })
            })
        }
    }
}


#[allow(dead_code)]
struct VulkanContext{
    context: DirectContext,
    library: Arc<VulkanLibrary>,
    instance: Arc<Instance>,
    physical_device: Arc<PhysicalDevice>,
    device: Arc<Device>,
    queue: Arc<Queue>,
    last_use: Instant,
}

impl VulkanContext{
    fn new() -> Result<Self, String> {
        let library = VulkanLibrary::new().or(Err("Vulkan libraries not found on system"))?;

        let instance = Instance::new(
            Arc::clone(&library),
            InstanceCreateInfo {
                flags: InstanceCreateFlags::ENUMERATE_PORTABILITY,
                ..Default::default()
            },
        )
        .or(Err("Could not create Vulkan instance"))?;

        let (physical_device, queue_family_index) = instance
            .enumerate_physical_devices()
            .or(Err("Vulkan: No physical devices found"))?
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
            .ok_or("No suitable Vulkan physical device found")?;

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
        .or(Err("Failed to create Vulkan device"))?;

        let queue = queues.next().ok_or("Failed to create Vulkan graphics queue")?;

        let context = {
            let get_proc = |of| unsafe {
                match of {
                    GetProcOf::Instance(instance, name) => {
                        let vk_instance = ash::vk::Instance::from_raw(instance as _);
                        library.get_instance_proc_addr(vk_instance, name)
                    }
                    GetProcOf::Device(device, name) => {
                        let get_device_proc_addr = instance.fns().v1_0.get_device_proc_addr;
                        let vk_device = ash::vk::Device::from_raw(device as _);
                        get_device_proc_addr(vk_device, name)
                    }
                }
                .map(|f| f as _)
                .unwrap_or_else(|| {
                    println!("Failed to resolve Vulkan proc `{}`", of.name().to_str().unwrap());
                    ptr::null()
                })
            };
            let backend_context = unsafe {
                BackendContext::new(
                    instance.handle().as_raw() as _,
                    physical_device.handle().as_raw() as _,
                    device.handle().as_raw() as _,
                    (
                        queue.handle().as_raw() as _,
                        queue.queue_family_index() as usize,
                    ),
                    &get_proc,
                )
            };
            direct_contexts::make_vulkan(&backend_context, None)
        }
        .ok_or("Failed to create Vulkan backend context")?;

        Ok(Self {
            context,
            library,
            instance,
            physical_device,
            device,
            queue,
            last_use: Instant::now() + VK_CONTEXT_LIFESPAN
        })
    }

    pub fn works(&mut self) -> bool{
        self.surface(&ImageInfo::new_n32_premul(
            ISize::new(100, 100),
            Some(ColorSpace::new_srgb()),
        )).is_some()
    }

    pub fn surface(&mut self, image_info: &ImageInfo) -> Option<Surface> {
        self.last_use = Instant::now();
        surfaces::render_target(
            &mut self.context,
            Budgeted::Yes,
            image_info,
            Some(4),
            SurfaceOrigin::BottomLeft,
            None,
            false,
            None,
        )
    }

}