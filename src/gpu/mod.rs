use skia_safe::gpu::{DirectContext, SurfaceOrigin};
use skia_safe::{Budgeted, ImageInfo, Surface};

#[cfg(target_os = "macos")]
use crate::gpu::metal::Metal as Engine;

#[cfg(target_os = "macos")]
mod metal;

#[cfg(not(target_os = "macos"))]
mod vulkan;

#[cfg(not(target_os = "macos"))]
use crate::gpu::vulkan::Vulkan as Engine;


// mod gl;

#[derive(Copy, Clone, Debug)]
pub enum RenderingEngine{
    CPU,
    GPU,
}

impl Default for RenderingEngine {
    fn default() -> Self {
        if Engine::supported() { Self::GPU } else { Self::CPU }
    }
}

impl RenderingEngine{
    pub fn supported(&self) -> bool {
        match self {
            Self::GPU => Engine::supported(),
            Self::CPU => true
        }
    }

    pub fn get_surface(&self, image_info: &ImageInfo) -> Option<Surface> {
        if let Some(mut context) = self.get_direct_context() {
            Surface::new_render_target(
                &mut context,
                Budgeted::Yes,
                image_info,
                Some(4),
                SurfaceOrigin::BottomLeft,
                None,
                true,
            )
        } else {
            Surface::new_raster(image_info, None, None)
        }
    }

    fn get_direct_context(&self) -> Option<DirectContext> {
        match self {
            Self::GPU => Engine::direct_context(),
            Self::CPU => None
        }
    }
}
