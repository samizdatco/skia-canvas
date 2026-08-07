use std::{cell::RefCell, sync::{Arc, OnceLock}, time::{Instant, Duration}};
use serde_json::{json, Value};
use vulkano::{
    device::{
        physical::{PhysicalDevice, PhysicalDeviceType},
        Device, DeviceCreateInfo, Queue, QueueCreateInfo,
    },
    instance::Instance,
    VulkanLibrary,
};
use skia_safe::{
    gpu::{ surfaces, Budgeted, DirectContext, SurfaceOrigin },
    ColorSpace, ISize, ImageInfo, Surface,
};

use crate::gfx::page::ExportOptions;
use super::{VulkanShared, make_direct_context};

thread_local!( static VK_CONTEXT: RefCell<Option<VulkanContext>> = const { RefCell::new(None) }; );
static VK_STATUS: OnceLock<Value> = OnceLock::new();
static VK_CONTEXT_LIFESPAN:Duration = Duration::from_secs(5); // rebuilds can take ~70ms on a 3060, so use a long lifespan

pub struct VulkanEngine;

impl VulkanEngine {
    pub fn api() -> Option<String>{
        Some("Vulkan".to_string())
    }

    pub fn supported() -> bool {
        Self::status()["renderer"] == "GPU"
    }

    pub fn status() -> Value {
        VK_STATUS.get_or_init(||{
            match VulkanContext::new() {
                // if a context can successfully be created, collect info about the GPU
                Ok(context) => {
                    let device_props = context.physical_device.properties();
                    let gpu_type = match device_props.device_type {
                        PhysicalDeviceType::IntegratedGpu => Some("Integrated GPU"),
                        PhysicalDeviceType::DiscreteGpu => Some("Discrete GPU"),
                        PhysicalDeviceType::VirtualGpu => Some("Virtual GPU"),
                        _ => Some("Software Rasterizer")
                    };

                    json!({
                        "renderer": "GPU",
                        "api": "Vulkan",
                        "device": gpu_type.map(|t| format!("{} ({})",
                            t, device_props.device_name)
                        ),
                        "driver":format!("{} ({})",
                            device_props.driver_id.map(|id| format!("{:?}", id) ).unwrap_or("Unknown Driver".to_string()),
                            device_props.driver_info.as_ref().unwrap_or(&"Unknown Version".to_string()),
                        ),
                        "threads": rayon::current_num_threads(),
                    })
                },
                // if no working GPUs are available, report the error received
                Err(msg) => json!({
                    "renderer": "CPU",
                    "api": "Vulkan",
                    "device": "CPU-based renderer (Fallback)",
                    "driver": "N/A",
                    "threads": rayon::current_num_threads(),
                    "error": msg,
                })
            }
        }).clone()
    }

    pub fn with_context<T, F>(f:F) -> Result<T, String>
        where F:FnOnce(&mut VulkanContext) -> Result<T, String>
    {
        match VulkanEngine::supported() {
            false => Err("Vulkan API not supported".to_string()),
            true => VK_CONTEXT.with_borrow_mut(|local_ctx|{
                local_ctx
                    // lazily initialize this thread's context...
                    .take()
                    .or_else(|| VulkanContext::new().ok() )
                    .ok_or("Vulkan: Initialization failed".to_string())
                    .and_then(|ctx|{
                        f(local_ctx.insert(ctx))
                    })
            })
        }
    }

    pub fn with_direct_context<F>(f:F)
        where F:FnOnce(Option<&mut DirectContext>)
    {
        Self::with_context(|ctx| Ok(f(Some(&mut ctx.context))) ).ok();
    }


    pub fn make_surface(image_info: &ImageInfo, opts:&ExportOptions, budgeted:bool) -> Result<Surface, String>{
        Self::with_context(|ctx| ctx.surface(image_info, opts, budgeted) )
    }

    // allow the render thread to check how long the context has gone unused
    pub fn context_is_idle() -> bool{
        VK_CONTEXT.with_borrow(|cell|
            cell.as_ref().map(|engine| engine.last_use.elapsed() > VK_CONTEXT_LIFESPAN).unwrap_or(false)
        )
    }

    pub fn retire(){
        VK_CONTEXT.with_borrow_mut(|cell| { cell.take(); });
    }

    // called by the render thread between jobs to expire skia's internal caches
    pub fn purge_stale(){
        VK_CONTEXT.with_borrow_mut(|cell| {
            if let Some(engine) = cell.as_mut(){
                engine.context.perform_deferred_cleanup(Duration::from_secs(1), None);
            }
        });
    }

    pub fn with_cleanup<T>(f: impl FnOnce() -> T) -> T { f() }
}


#[allow(dead_code)]
pub struct VulkanContext{
    context: DirectContext,
    library: Arc<VulkanLibrary>,
    instance: Arc<Instance>,
    physical_device: Arc<PhysicalDevice>,
    device: Arc<Device>,
    queue: Arc<Queue>,
    msaa: Vec<usize>,
    last_use: Instant,
}

impl VulkanContext{
    fn new() -> Result<Self, String> {
        let shared = VulkanShared::get()?;
        let mut failure = "Vulkan: No suitable physical device found".to_string();

        // walk the rank-ordered candidates and keep the first one that initializes *and* can actually render
        for (physical_device, queue_family_index) in shared.offscreen_devices(){
            let device_name = physical_device.properties().device_name.clone();
            match Self::for_device(physical_device, queue_family_index){
                Ok(mut context) => match context.works(){
                    true => return Ok(context),
                    false => failure = format!("Vulkan: {device_name} was instantiated but unable to render"),
                },
                Err(msg) => failure = format!("Vulkan: {device_name}: {msg}"),
            }
        }

        Err(failure)
    }

    fn for_device(physical_device:Arc<PhysicalDevice>, queue_family_index:u32) -> Result<Self, String> {
        let shared = VulkanShared::get()?;
        let library = shared.library.clone();
        let instance = shared.instance.clone();

        // ...but create a private logical device, queue, and DirectContext
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

        let queue = queues.next().ok_or("Vulkan: Failed to create graphics queue")?;

        let context = make_direct_context(&device, &queue)
            .ok_or("Vulkan: Failed to create backend context")?;

        let vk_sample_counts = physical_device.properties().framebuffer_color_sample_counts;
        let max_sample_count = context.max_surface_sample_count_for_color_type(
            // even if the device claims it supports >1 samples, let skia overrule it
            ImageInfo::new_n32_premul((0,0), None).color_type()
        );
        let mut msaa:Vec<usize> = [1,2,4,8,16,32].into_iter()
            .filter(|s| s <= &max_sample_count)
            .filter_map(|s| vulkano::image::SampleCount::try_from(s as u32).ok() )
            .filter(|s| vk_sample_counts.contains_enum(*s) )
            .map(|s| s as usize)
            .collect();

        msaa.insert(0, 0); // also include the shader-based AA option

        Ok(Self {
            context,
            library,
            instance,
            physical_device,
            device,
            queue,
            msaa,
            last_use: Instant::now() + VK_CONTEXT_LIFESPAN
        })
    }

    pub fn works(&mut self) -> bool{
        self.surface(&ImageInfo::new_n32_premul(
            ISize::new(1, 1),
            Some(ColorSpace::new_srgb()),
        ), &ExportOptions::default(), true).is_ok()
    }

    pub fn surface(&mut self, image_info: &ImageInfo, opts:&ExportOptions, budgeted:bool) -> Result<Surface, String> {
        self.last_use = Instant::now();
        surfaces::render_target(
            &mut self.context,
            if budgeted { Budgeted::Yes } else { Budgeted::No },
            image_info,
            Some(opts.msaa_from(&self.msaa)?),
            SurfaceOrigin::BottomLeft,
            Some(&opts.surface_props()),
            false,
            None,
        ).ok_or(
            format!("Could not allocate new {}×{} bitmap", image_info.width(), image_info.height())
        )
    }

}
