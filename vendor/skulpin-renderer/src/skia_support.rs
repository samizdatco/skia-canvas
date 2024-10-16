use rafx::api::*;
use rafx::framework::*;
use rafx::api::ash;
use ash::vk;
use ash::version::InstanceV1_0;
use rafx::api::vulkan::RafxRawImageVulkan;

/// Handles setting up skia to use the same vulkan instance we initialize
pub struct VkSkiaContext {
    pub context: skia_safe::gpu::DirectContext,
}

impl VkSkiaContext {
    pub fn new(
        device_context: &RafxDeviceContext,
        queue: &RafxQueue,
    ) -> Self {
        use vk::Handle;

        let vk_device_context = device_context.vk_device_context().unwrap();
        let entry = vk_device_context.entry();
        let instance = vk_device_context.instance();
        let physical_device = vk_device_context.physical_device();
        let device = vk_device_context.device();

        let graphics_queue_family = vk_device_context
            .queue_family_indices()
            .graphics_queue_family_index;

        let get_proc = |of| unsafe {
            match Self::get_proc(instance, entry, of) {
                Some(f) => f as _,
                None => {
                    error!("resolve of {} failed", of.name().to_str().unwrap());
                    std::ptr::null()
                }
            }
        };

        info!(
            "Setting up skia backend context with queue family index {}",
            graphics_queue_family
        );

        let backend_context = unsafe {
            let vk_queue_handle = *queue.vk_queue().unwrap().queue().queue().lock().unwrap();
            skia_safe::gpu::vk::BackendContext::new(
                instance.handle().as_raw() as _,
                physical_device.as_raw() as _,
                device.handle().as_raw() as _,
                (
                    vk_queue_handle.as_raw() as _,
                    graphics_queue_family as usize,
                ),
                &get_proc,
            )
        };

        let context = skia_safe::gpu::DirectContext::new_vulkan(&backend_context, None).unwrap();

        VkSkiaContext { context }
    }

    unsafe fn get_proc<E: ash::version::EntryV1_0>(
        instance: &ash::Instance,
        entry: &E,
        of: skia_safe::gpu::vk::GetProcOf,
    ) -> vk::PFN_vkVoidFunction {
        use rafx::api::ash::vk::Handle;
        match of {
            skia_safe::gpu::vk::GetProcOf::Instance(instance_proc, name) => {
                // Instead of forcing skia to use vk 1.0.0,
                // use the instance version that is the most appropriate.
                let ash_instance = vk::Instance::from_raw(instance_proc as _);
                entry.get_instance_proc_addr(ash_instance, name)
            }
            skia_safe::gpu::vk::GetProcOf::Device(device_proc, name) => {
                let ash_device = vk::Device::from_raw(device_proc as _);
                instance.get_device_proc_addr(ash_device, name)
            }
        }
    }
}

/// Wraps a skia surface/canvas that can be drawn on and makes the vulkan resources accessible
pub struct VkSkiaSurface {
    pub device_context: RafxDeviceContext,
    pub image_view: ResourceArc<ImageViewResource>,
    pub surface: skia_safe::Surface,
    pub texture: skia_safe::gpu::BackendTexture,
}

impl VkSkiaSurface {
    pub fn get_image_from_skia_texture(texture: &skia_safe::gpu::BackendTexture) -> vk::Image {
        unsafe { std::mem::transmute(texture.vulkan_image_info().unwrap().image.as_ref().unwrap()) }
    }

    pub fn new(
        resource_manager: &ResourceManager,
        context: &mut VkSkiaContext,
        extents: RafxExtents2D,
    ) -> RafxResult<Self> {
        assert!(extents.width > 0);
        assert!(extents.height > 0);
        // The "native" color type is based on platform. For example, on Windows it's BGR and on
        // MacOS it's RGB
        let color_type = skia_safe::ColorType::N32;
        let alpha_type = skia_safe::AlphaType::Premul;
        let color_space = Some(skia_safe::ColorSpace::new_srgb_linear());

        let image_info = skia_safe::ImageInfo::new(
            (extents.width as i32, extents.height as i32),
            color_type,
            alpha_type,
            color_space,
        );

        let mut surface = skia_safe::Surface::new_render_target(
            &mut context.context,
            skia_safe::Budgeted::Yes,
            &image_info,
            None,
            skia_safe::gpu::SurfaceOrigin::TopLeft,
            None,
            false,
        )
        .unwrap();

        let texture = surface
            .get_backend_texture(skia_safe::surface::BackendHandleAccess::FlushRead)
            .unwrap();
        let image = Self::get_image_from_skia_texture(&texture);

        // According to docs, kN32_SkColorType can only be kRGBA_8888_SkColorType or
        // kBGRA_8888_SkColorType. Whatever it is, we need to set up the image view with the
        // matching format
        let format = match color_type {
            skia_safe::ColorType::RGBA8888 => RafxFormat::R8G8B8A8_UNORM,
            skia_safe::ColorType::BGRA8888 => RafxFormat::B8G8R8A8_UNORM,
            _ => {
                warn!("Unexpected native color type {:?}", color_type);
                RafxFormat::R8G8B8A8_UNORM
            }
        };

        let device_context = resource_manager.device_context();

        let raw_image = RafxRawImageVulkan {
            allocation: None,
            image,
        };

        let image = rafx::api::vulkan::RafxTextureVulkan::from_existing(
            device_context.vk_device_context().unwrap(),
            Some(raw_image),
            &RafxTextureDef {
                extents: RafxExtents3D {
                    width: extents.width,
                    height: extents.height,
                    depth: 1,
                },
                format,
                resource_type: RafxResourceType::TEXTURE,
                sample_count: RafxSampleCount::SampleCount1,
                ..Default::default()
            },
        )?;

        let image = resource_manager
            .resources()
            .insert_image(RafxTexture::Vk(image));
        let image_view = resource_manager
            .resources()
            .get_or_create_image_view(&image, None)?;

        Ok(VkSkiaSurface {
            device_context: device_context.clone(),
            surface,
            texture,
            image_view,
        })
    }
}
