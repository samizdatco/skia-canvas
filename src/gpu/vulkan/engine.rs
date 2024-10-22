#![allow(dead_code)]
// #![allow(unused_imports)]
use std::ptr;
use std::cell::RefCell;
use std::sync::Arc;

use vulkano::{
    VulkanLibrary, VulkanObject, Handle,
    device::{
        physical::{PhysicalDevice, PhysicalDeviceType}, Device, DeviceCreateInfo, 
        QueueCreateInfo, QueueFlags, Queue
    },
    instance::{Instance, InstanceCreateFlags, InstanceCreateInfo},
};

use skia_safe::{ImageInfo, ISize, Surface, ColorSpace};
use skia_safe::gpu::{direct_contexts, surfaces, DirectContext, Budgeted, SurfaceOrigin};
use skia_safe::gpu::vk::{BackendContext, GetProcOf};

thread_local!(
    static VK_CONTEXT: RefCell<Option<VulkanEngine>> = const { RefCell::new(None) };
    static VK_STATUS: RefCell<Option<String>> = const { RefCell::new(None) };
);

pub struct VulkanEngine {
    context: DirectContext,
    library: Arc<VulkanLibrary>,
    instance: Arc<Instance>,
    physical_device: Arc<PhysicalDevice>,
    device: Arc<Device>,
    queue: Arc<Queue>,
}

impl VulkanEngine {
    fn init(){
        VK_CONTEXT.with(|cell| {
            let mut local_ctx = cell.borrow_mut();
            if local_ctx.is_none() {
                match VulkanEngine::new(){
                    Ok(ctx) => {
                        local_ctx.replace(ctx);
                    },
                    Err(msg) => {
                        Self::set_status(msg);
                    }
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
        let library = VulkanLibrary::new().or(Err("Vulkan: not installed"))?;

        let instance = Instance::new(
            Arc::clone(&library),
            InstanceCreateInfo {
                flags: InstanceCreateFlags::ENUMERATE_PORTABILITY,
                ..Default::default()
            },
        )
        .or(Err("Vulkan: Could not create instance"))?;
    
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
            .ok_or("Vulkan: No suitable physical device found")?;
    
        Self::set_status(format!(
                "Vulkan on {} ({:?})",
                physical_device.properties().device_name,
                physical_device.properties().device_type,
            )
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
        .or(Err("Vulkan: Failed to create device"))?;
        
        let queue = queues.next().ok_or("Vulkan: Failed to create queue")?;

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
                .map(|f| f as _ )
                .unwrap_or_else(||{
                    println!("Vulkan: failed to resolve {}", of.name().to_str().unwrap());
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
                        queue.queue_family_index() as usize
                    ),
                    &get_proc,
                )
            };
            direct_contexts::make_vulkan(&backend_context, None)
        }.ok_or("Vulkan: Failed to create context")?;

        Ok(Self { context, library, instance, physical_device, device, queue })
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
    
    
    pub fn set_status(msg:String) {
        VK_STATUS.with(|status| status.borrow_mut().replace(msg) );
    }

    pub fn status() -> Option<String>{
        VK_STATUS.with(|err_cell| err_cell.borrow().clone())
    }
}
