use std::{ptr, sync::{Arc, OnceLock}};
use ash::vk::Handle;
use vulkano::{
    device::{
        physical::{PhysicalDevice, PhysicalDeviceType},
        Device, Queue, QueueFlags,
    },
    format::Format as VkFormat,
    instance::{Instance, InstanceCreateFlags, InstanceCreateInfo, InstanceExtensions},
    Version, VulkanLibrary, VulkanObject,
};
#[cfg(feature = "window")]
use vulkano::swapchain::Surface;
use skia_safe::{ gpu::{vk, direct_contexts, DirectContext}, ColorType };

pub mod offscreen;

#[cfg(feature = "window")]
pub mod window;

static VK_FORMATS: &'static [VkFormat] = &[
    VkFormat::R8G8B8A8_UNORM,
    VkFormat::R8G8B8A8_SRGB,
    VkFormat::R8_UNORM,
    VkFormat::B8G8R8A8_UNORM,
    VkFormat::R5G6B5_UNORM_PACK16,
    VkFormat::B5G6R5_UNORM_PACK16,
    VkFormat::R16G16B16A16_SFLOAT,
    VkFormat::R16_SFLOAT,
    VkFormat::R8G8B8_UNORM,
    VkFormat::R8G8_UNORM,
    VkFormat::A2B10G10R10_UNORM_PACK32,
    VkFormat::A2R10G10B10_UNORM_PACK32,
    VkFormat::R10X6G10X6B10X6A10X6_UNORM_4PACK16,
    VkFormat::B4G4R4A4_UNORM_PACK16,
    VkFormat::R4G4B4A4_UNORM_PACK16,
    VkFormat::R16_UNORM,
    VkFormat::R16G16_UNORM,
    VkFormat::G8_B8_R8_3PLANE_420_UNORM,
    VkFormat::G8_B8R8_2PLANE_420_UNORM,
    VkFormat::G10X6_B10X6R10X6_2PLANE_420_UNORM_3PACK16,
    VkFormat::R16G16B16A16_UNORM,
    VkFormat::R16G16_SFLOAT,
];

fn to_sk_format(vulkano_format:&VkFormat) -> Option<(vk::Format, ColorType)>{
    // Format / ColorType pairs
    // https://github.com/google/skia/blob/4f24819404272433687a76e407bcd7877384f512/src/gpu/ganesh/vk/GrVkCaps.cpp#L880
    //
    // GrColorType -> SkColorType mappings
    // https://github.com/google/skia/blob/4f24819404272433687a76e407bcd7877384f512/include/private/gpu/ganesh/GrTypesPriv.h#L590
    //
    // Present in the GrVkCaps 'supported' list but lacking supported GrColorTypes so omitted:
    // - VkFormat::ETC2_R8G8B8_UNORM_BLOCK
    // - VkFormat::BC1_RGB_UNORM_BLOCK
    // - VkFormat::BC1_RGBA_UNORM_BLOCK
    match vulkano_format {
        VkFormat::R8G8B8A8_UNORM => Some(( vk::Format::R8G8B8A8_UNORM, ColorType::RGBA8888 )),
        VkFormat::R8G8B8A8_SRGB => Some(( vk::Format::R8G8B8A8_SRGB, ColorType::SRGBA8888 )),
        VkFormat::R8_UNORM => Some(( vk::Format::R8_UNORM, ColorType::R8UNorm )),
        VkFormat::B8G8R8A8_UNORM => Some(( vk::Format::B8G8R8A8_UNORM, ColorType::BGRA8888 )),
        VkFormat::R5G6B5_UNORM_PACK16 => Some(( vk::Format::R5G6B5_UNORM_PACK16, ColorType::RGB565 )),
        VkFormat::B5G6R5_UNORM_PACK16 => Some(( vk::Format::B5G6R5_UNORM_PACK16, ColorType::RGB565 )),
        VkFormat::R16G16B16A16_SFLOAT => Some(( vk::Format::R16G16B16A16_SFLOAT, ColorType::RGBAF16 )),
        VkFormat::R16_SFLOAT => Some(( vk::Format::R16_SFLOAT, ColorType::A16Float )),
        VkFormat::R8G8B8_UNORM => Some(( vk::Format::R8G8B8_UNORM, ColorType::RGB888x )),
        VkFormat::R8G8_UNORM => Some(( vk::Format::R8G8_UNORM, ColorType::R8G8UNorm )),
        VkFormat::A2B10G10R10_UNORM_PACK32 => Some(( vk::Format::A2B10G10R10_UNORM_PACK32, ColorType::RGBA1010102 )),
        VkFormat::A2R10G10B10_UNORM_PACK32 => Some(( vk::Format::A2R10G10B10_UNORM_PACK32, ColorType::BGRA1010102 )),
        VkFormat::R10X6G10X6B10X6A10X6_UNORM_4PACK16 => Some(( vk::Format::R10X6G10X6B10X6A10X6_UNORM_4PACK16, ColorType::RGBA10x6 )),
        VkFormat::B4G4R4A4_UNORM_PACK16 => Some(( vk::Format::B4G4R4A4_UNORM_PACK16, ColorType::ARGB4444 )),
        VkFormat::R4G4B4A4_UNORM_PACK16 => Some(( vk::Format::R4G4B4A4_UNORM_PACK16, ColorType::ARGB4444 )),
        VkFormat::R16_UNORM => Some(( vk::Format::R16_UNORM, ColorType::A16UNorm )),
        VkFormat::R16G16_UNORM => Some(( vk::Format::R16G16_UNORM, ColorType::R16G16UNorm )),
        VkFormat::G8_B8_R8_3PLANE_420_UNORM => Some(( vk::Format::G8_B8_R8_3PLANE_420_UNORM, ColorType::RGB888x )),
        VkFormat::G8_B8R8_2PLANE_420_UNORM => Some(( vk::Format::G8_B8R8_2PLANE_420_UNORM, ColorType::RGB888x )),
        VkFormat::G10X6_B10X6R10X6_2PLANE_420_UNORM_3PACK16 => Some(( vk::Format::G10X6_B10X6R10X6_2PLANE_420_UNORM_3PACK16, ColorType::RGBA1010102 )),
        VkFormat::R16G16B16A16_UNORM => Some(( vk::Format::R16G16B16A16_UNORM, ColorType::R16G16B16A16UNorm )),
        VkFormat::R16G16_SFLOAT => Some(( vk::Format::R16G16_SFLOAT, ColorType::R16G16Float )),
        _ => None
    }
}

// immutable base components that can be shared across threads
struct VulkanShared {
    library: Arc<VulkanLibrary>,
    instance: Arc<Instance>,
    offscreen_candidates: Vec<(Arc<PhysicalDevice>, u32)>,
}

static VK_SHARED: OnceLock<Result<VulkanShared, String>> = OnceLock::new();

impl VulkanShared {
    fn get() -> Result<&'static VulkanShared, String> {
        VK_SHARED.get_or_init(VulkanShared::new).as_ref().map_err(|err| err.clone())
    }

    fn new() -> Result<Self, String> {
        let library = VulkanLibrary::new().or(Err("Vulkan libraries not found on system"))?;

        // include window extensions (if enabled) so the instance can also be used for onscreen renderers
        let enabled_extensions = {
            #[cfg(feature = "window")]
            {
                let mut wsi = InstanceExtensions{ khr_surface: true, ..InstanceExtensions::empty() };
                #[cfg(target_os = "macos")] {
                  wsi.ext_metal_surface = true;
                }
                #[cfg(target_os = "linux")] {
                    wsi.khr_xlib_surface = true;
                    wsi.khr_xcb_surface = true;
                    wsi.khr_wayland_surface = true;
                }
                #[cfg(target_os = "windows")] {
                  wsi.khr_win32_surface = true;
                }
                wsi.intersection(library.supported_extensions())
            }
            #[cfg(not(feature = "window"))]
            { InstanceExtensions::empty() }
        };

        let instance = Instance::new(
            library.clone(),
            InstanceCreateInfo {
                flags: InstanceCreateFlags::ENUMERATE_PORTABILITY, // support MoltenVK
                enabled_extensions,
                ..Default::default()
            },
        ).or(Err("Vulkan: Could not create instance"))?;

        // collect the offscreen candidates that will be reused for all exports
        let mut offscreen_candidates:Vec<_> = instance
            .enumerate_physical_devices()
            .or(Err("Vulkan: No physical devices found"))?
            .filter(|p| p.api_version() >= SKIA_MIN_VULKAN)
            .filter_map(|p| {
                // find a graphics-capable queue family, skipping devices that lack one
                p.queue_family_properties()
                    .iter()
                    .position(|q| q.queue_flags.intersects(QueueFlags::GRAPHICS))
                    .map(|i| (p, i as u32))
            })
            .collect();
        offscreen_candidates.sort_by_key(|(p, _)| device_type_rank(p));

        if offscreen_candidates.is_empty(){
            Err("Vulkan: No suitable physical device found")?
        }

        Ok(Self{ library, instance, offscreen_candidates })
    }

    // lists offscreen devices (no surface needed), sorted by device class. each device needs to be tested
    // before being settled on since it's not guaranteed any listed device can actually build a context.
    fn offscreen_devices(&self) -> impl Iterator<Item = (Arc<PhysicalDevice>, u32)> + '_ {
        self.offscreen_candidates.iter().cloned()
    }

    // lists devices that can present to *this* window's surface (which may vary for hybrid-gpu &
    // multi-screen setups). computed fresh per-surface rather than cached, but ranked by device class
    // and tried in order until a working device can be found.
    #[cfg(feature = "window")]
    fn screen_devices(&self, surface:&Arc<Surface>) -> impl Iterator<Item = (Arc<PhysicalDevice>, u32)> {
        let mut candidates:Vec<_> = self.instance
            .enumerate_physical_devices()
            .map(|devices| devices
                .filter(|p| p.api_version() >= SKIA_MIN_VULKAN)
                .filter(|p| p.supported_extensions().khr_swapchain)
                .filter_map(|p| {
                    // need a queue family that is both graphics-capable and can present to this surface
                    p.queue_family_properties()
                        .iter()
                        .enumerate()
                        .position(|(i, q)|
                            q.queue_flags.intersects(QueueFlags::GRAPHICS)
                                && p.surface_support(i as u32, surface).unwrap_or(false)
                        )
                        .map(|i| (p, i as u32))
                })
                .collect::<Vec<_>>()
            )
            .unwrap_or_default();
        candidates.sort_by_key(|(p, _)| device_type_rank(p));
        candidates.into_iter()
    }
}

// Skia's Vulkan backend requires Vulkan 1.1 as its minimum supported API version. This isn't
// exposed as a constant by skia-safe and is only documented in `gpu::vk::BackendContext::new`, so
// re-check against that doc on any skia-safe upgrade. When confronted with a device below the cutoff
// Skia will simply abort the whole process. See #289.
const SKIA_MIN_VULKAN: Version = Version::V1_1;

// choose the fastest general class of device
fn device_type_rank(device: &PhysicalDevice) -> u8 {
    match device.properties().device_type {
        PhysicalDeviceType::DiscreteGpu => 0,
        PhysicalDeviceType::IntegratedGpu => 1,
        PhysicalDeviceType::VirtualGpu => 2,
        PhysicalDeviceType::Cpu => 3,
        PhysicalDeviceType::Other => 4,
        _ => 5,
    }
}

// create a Skia rendering context for use by either on- or offscreen renderers
fn make_direct_context(device:&Arc<Device>, queue:&Arc<Queue>) -> Option<DirectContext>{
    let instance = device.instance();
    let library = instance.library();
    unsafe {
        let get_proc = |gpo| {
            let get_device_proc_addr = instance.fns().v1_0.get_device_proc_addr;
            match gpo {
                vk::GetProcOf::Instance(instance_handle, name) => {
                    let vk_instance = ash::vk::Instance::from_raw(instance_handle as _);
                    library.get_instance_proc_addr(vk_instance, name)
                }
                vk::GetProcOf::Device(device_handle, name) => {
                    let vk_device = ash::vk::Device::from_raw(device_handle as _);
                    get_device_proc_addr(vk_device, name)
                }
            }
            .map(|f| f as _)
            .unwrap_or_else(|| {
                eprintln!("Vulkan: failed to resolve {}", gpo.name().to_str().unwrap());
                ptr::null()
            })
        };

        // choose an API version explicitly. When set to None, Skia trusts the loader's version
        // (1.4 on current Windows) over the older one the instance negotiated (1.3), then probes
        // for newer core functions the instance never opted into. Lenient drivers (NVIDIA) will
        // tolerate this, but stricter ones (Intel) will crash [see #274].
        let vk_version = device.api_version();
        let max_api_version = vk::Version::from((
            vk_version.major as usize,
            vk_version.minor as usize,
            vk_version.patch as usize,
        ));

        let backend_context = vk::BackendContext::new_builder(
            instance.handle().as_raw() as _,
            device.physical_device().handle().as_raw() as _,
            device.handle().as_raw() as _,
            (
                queue.handle().as_raw() as _,
                queue.queue_family_index() as usize,
            ),
            &get_proc,
            Some(max_api_version),
        ).build();
        direct_contexts::make_vulkan(&backend_context, None)
    }
}
