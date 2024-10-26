#![allow(unused_imports)]
use std::{cell::RefCell, sync::{Arc, OnceLock}, ptr};
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

thread_local!(
    static VK_CONTEXT: RefCell<Option<VulkanEngine>> = const { RefCell::new(None) };
    static VK_STATUS: RefCell<Value> = const { RefCell::new(Value::Null) };
);

static IS_SUPPORTED: OnceLock<bool> = OnceLock::new();

#[derive(Debug)]
#[allow(dead_code)]
pub struct VulkanEngine {
    context: DirectContext,
    library: Arc<VulkanLibrary>,
    instance: Arc<Instance>,
    physical_device: Arc<PhysicalDevice>,
    device: Arc<Device>,
    queue: Arc<Queue>,
}

impl VulkanEngine {
    fn init() {
        VK_CONTEXT.with_borrow_mut(|local_ctx| {
            if local_ctx.is_none() {
                match VulkanEngine::new() {
                    Ok(ctx) => {
                        local_ctx.replace(ctx);
                    }
                    Err(msg) => {
                        Self::set_status(
                            json!({
                                "renderer": "CPU",
                                "api": "Vulkan",
                                "device": "CPU-based renderer (Fallback)",
                                "error": msg,
                            })
                        );                        
                    }
                }
            }
        })
    }

    pub fn supported() -> bool {
        Self::init();
        *IS_SUPPORTED.get_or_init(||
            VK_CONTEXT.with_borrow(|cell| cell.is_some())
                && Self::surface(&ImageInfo::new_n32_premul(
                    ISize::new(100, 100),
                    Some(ColorSpace::new_srgb()),
                ))
                .is_some()
        )
    }

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

        let (mode, gpu_type) = match physical_device.properties().device_type {
            PhysicalDeviceType::IntegratedGpu => ("GPU", Some("Integrated")),
            PhysicalDeviceType::DiscreteGpu => ("GPU", Some("Discrete")),
            PhysicalDeviceType::VirtualGpu => ("GPU", Some("Virtual")),
            _ => ("CPU", None)
        };

        Self::set_status(json!({
            "renderer": mode,
            "device": gpu_type.map(|t| format!("{} GPU ({})", 
                t, physical_device.properties().device_name)
            ),
            "api": "Vulkan",
            "error": Value::Null,
        }));
   
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
        })
    }

    pub fn surface(image_info: &ImageInfo) -> Option<Surface> {
        Self::init();
        VK_CONTEXT.with_borrow_mut(|cell| match cell {
            Some(engine) => 
                surfaces::render_target(
                    &mut engine.context,
                    Budgeted::Yes,
                    image_info,
                    Some(4),
                    SurfaceOrigin::BottomLeft,
                    None,
                    true,
                    None,
                ),
            _ => None
        })
    }

    pub fn set_status(msg: Value) {
        VK_STATUS.with_borrow_mut(|status| *status = msg);
    }

    pub fn status() -> Value {
        VK_STATUS.with_borrow(|err_cell| err_cell.clone() )
    }

}
