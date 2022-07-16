use std::ptr;
use std::cell::RefCell;
use std::ffi::CString;
use std::os::raw;

use ash::{Entry, Instance, vk};
use ash::vk::Handle;
use skia_safe::gpu;
use skia_safe::gpu::DirectContext;

thread_local!(static GL_CONTEXT: RefCell<Option<Vulkan>> = RefCell::new(None));


fn vulkan_init() -> bool {
    GL_CONTEXT.with(|cell| {
        let mut local_ctx = cell.borrow_mut();
        if local_ctx.is_none() {
            if let Ok(ctx) = Vulkan::new() {
                local_ctx.replace(ctx);
                true
            } else {
                false
            }
        } else {
            true
        }
    })
}

pub fn get_vulkan_context() -> DirectContext {
    vulkan_init();
    GL_CONTEXT.with(|cell| {
        let local_ctx = cell.borrow();
        local_ctx.as_ref().unwrap().context.clone()
    })
}

pub fn vulkan_supported() -> bool {
    vulkan_init()
}

pub struct Vulkan {
    context: gpu::DirectContext,
    _ash_graphics: AshGraphics,
}

impl Vulkan {
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
