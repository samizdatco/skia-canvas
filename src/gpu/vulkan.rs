use std::ptr;
use std::cell::RefCell;
use std::ffi::CString;
use std::os::raw;

use ash::{Entry, Instance, vk};
use ash::vk::Handle;
use skia_safe::gpu::{self, DirectContext, SurfaceOrigin};
use skia_safe::{ImageInfo, Budgeted, Surface};

use std::sync::{Arc, Mutex};
use skulpin::{CoordinateSystem, Renderer, RendererBuilder};
use skulpin::rafx::api::RafxExtents2D;

#[cfg(feature = "window")]
use winit::{
    dpi::{LogicalSize, PhysicalSize},
    window::{Window},
};

thread_local!(static VK_CONTEXT: RefCell<Option<VulkanEngine>> = RefCell::new(None));

pub struct VulkanEngine {
    context: gpu::DirectContext,
    _ash_graphics: AshGraphics,
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
        VK_CONTEXT.with(|cell| cell.borrow().is_some() )
    }

    fn new() -> Result<Self, String> {
        let ash_graphics = unsafe { AshGraphics::new("skia-canvas") }?;
        let context = {
            let get_proc = |of| unsafe {
                match ash_graphics.get_proc(of) {
                    Some(f) => f as _,
                    None => {
                        println!("resolve of {} failed", of.name().to_str().unwrap());
                        ptr::null()
                    }
                }
            };

            let backend_context = unsafe {
                gpu::vk::BackendContext::new(
                    ash_graphics.instance.handle().as_raw() as _,
                    ash_graphics.physical_device.as_raw() as _,
                    ash_graphics.device.handle().as_raw() as _,
                    (
                        ash_graphics.queue_and_index.0.as_raw() as _,
                        ash_graphics.queue_and_index.1,
                    ),
                    &get_proc,
                )
            };

            DirectContext::new_vulkan(&backend_context, None)
        }.ok_or("Failed to create Vulkan context")?;

        Ok(Self {
            context,
            _ash_graphics: ash_graphics,
        })
    }

    pub fn surface(image_info: &ImageInfo) -> Option<Surface> {
        Self::init();
        VK_CONTEXT.with(|cell| {
            let local_ctx = cell.borrow();
            let mut context = local_ctx.as_ref().unwrap().context.clone();
            Surface::new_render_target(
                &mut context,
                Budgeted::Yes,
                image_info,
                Some(4),
                SurfaceOrigin::BottomLeft,
                None,
                true,
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

    pub fn draw<F: FnOnce(&mut skia_safe::Canvas, LogicalSize<f32>)>(
        &mut self,
        window: &Window,
        f: F,
    ) -> Result<(), String> {

        let size = window.inner_size();
        let window_extents = RafxExtents2D {
            width: size.width,
            height: size.height,
        };

        if let Err(e) = self.skulpin.lock().unwrap().draw(
            window_extents,
            window.scale_factor(),
            |canvas, coords| {
                let size = coords.window_logical_size();
                f(canvas, LogicalSize::new(size.width as f32, size.height as f32))
            })
        {
            Err(format!("Rendering error {:?}", e))
        }else{
            Ok(())
        }


    }
}


pub struct AshGraphics {
    pub entry: Entry,
    pub instance: Instance,
    pub physical_device: vk::PhysicalDevice,
    pub device: ash::Device,
    pub queue_and_index: (vk::Queue, usize),
}

impl Drop for AshGraphics {
    fn drop(&mut self) {
        unsafe {
            self.device.device_wait_idle().unwrap();
            self.device.destroy_device(None);
            self.instance.destroy_instance(None);
        }
    }
}

impl AshGraphics {
    pub fn vulkan_version() -> Option<(usize, usize, usize)> {
        let entry = unsafe { Entry::load() }.unwrap();

        let detected_version = entry.try_enumerate_instance_version().unwrap_or(None);

        detected_version.map(|ver| {
            (
                vk::api_version_major(ver).try_into().unwrap(),
                vk::api_version_minor(ver).try_into().unwrap(),
                vk::api_version_patch(ver).try_into().unwrap(),
            )
        })
    }

    pub unsafe fn new(app_name: &str) -> Result<AshGraphics, String> {
        let entry = Entry::load().or(Err("Failed to load Vulkan entry"))?;

        let minimum_version = vk::make_api_version(0, 1, 0, 0);

        let instance: Instance = {
            let api_version = Self::vulkan_version()
                .map(|(major, minor, patch)| {
                    vk::make_api_version(
                        0,
                        major.try_into().unwrap(),
                        minor.try_into().unwrap(),
                        patch.try_into().unwrap(),
                    )
                })
                .unwrap_or(minimum_version);

            let app_name = CString::new(app_name).unwrap();
            let layer_names: [&CString; 0] = []; // [CString::new("VK_LAYER_LUNARG_standard_validation").unwrap()];
            let extension_names_raw = []; // extension_names();

            let app_info = vk::ApplicationInfo::builder()
                .application_name(&app_name)
                .application_version(0)
                .engine_name(&app_name)
                .engine_version(0)
                .api_version(api_version);

            let layers_names_raw: Vec<*const raw::c_char> = layer_names
                .iter()
                .map(|raw_name| raw_name.as_ptr())
                .collect();

            let create_info = vk::InstanceCreateInfo::builder()
                .application_info(&app_info)
                .enabled_layer_names(&layers_names_raw)
                .enabled_extension_names(&extension_names_raw);

            entry
                .create_instance(&create_info, None)
        }.or(Err("Failed to create a Vulkan instance."))?;

        let (physical_device, queue_family_index) = {
            let physical_devices = instance
                .enumerate_physical_devices()
                .expect("Failed to enumerate Vulkan physical devices.");

            physical_devices
                .iter()
                .map(|physical_device| {
                    instance
                        .get_physical_device_queue_family_properties(*physical_device)
                        .iter()
                        .enumerate()
                        .find_map(|(index, info)| {
                            let supports_graphic =
                                info.queue_flags.contains(vk::QueueFlags::GRAPHICS);
                            if supports_graphic {
                                Some((*physical_device, index))
                            } else {
                                None
                            }
                        })
                })
                .find_map(|v| v)
        }.ok_or("Failed to find a Vulkan physical device.")?;

        let device: ash::Device = {
            let features = vk::PhysicalDeviceFeatures::default();

            let priorities = [1.0];

            let queue_info = [vk::DeviceQueueCreateInfo::builder()
                .queue_family_index(queue_family_index as _)
                .queue_priorities(&priorities)
                .build()];

            let device_extension_names_raw = [];

            let device_create_info = vk::DeviceCreateInfo::builder()
                .queue_create_infos(&queue_info)
                .enabled_extension_names(&device_extension_names_raw)
                .enabled_features(&features);

            instance
                .create_device(physical_device, &device_create_info, None)
        }.or(Err("Failed to create Device."))?;

        let queue_index: usize = 0;
        let queue: vk::Queue = device.get_device_queue(queue_family_index as _, queue_index as _);

        Ok(AshGraphics {
            queue_and_index: (queue, queue_index),
            device,
            physical_device,
            instance,
            entry,
        })
    }

    pub unsafe fn get_proc(&self, of: gpu::vk::GetProcOf) -> Option<unsafe extern "system" fn()> {
        match of {
            gpu::vk::GetProcOf::Instance(instance, name) => {
                let ash_instance = vk::Instance::from_raw(instance as _);
                self.entry.get_instance_proc_addr(ash_instance, name)
            }
            gpu::vk::GetProcOf::Device(device, name) => {
                let ash_device = vk::Device::from_raw(device as _);
                self.instance.get_device_proc_addr(ash_device, name)
            }
        }
    }
}
