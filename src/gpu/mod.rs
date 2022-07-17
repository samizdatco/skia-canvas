use skia_safe::gpu::{DirectContext, SurfaceOrigin};
use skia_safe::{Budgeted, ImageInfo, Surface};
use crate::gpu::gl::OpenGL;
use crate::gpu::vulkan::Vulkan;

mod vulkan;
mod gl;

#[derive(Copy, Clone, Debug)]
pub enum RenderingEngine{
    CPU, GL, VULKAN
}

impl Default for RenderingEngine {
    fn default() -> Self {
        for engine in [Self::VULKAN, Self::GL]{
            if engine.supported(){ return engine }
        }
        return Self::CPU
    }
}

impl RenderingEngine{
    pub fn supported(&self) -> bool {
        match self {
            Self::GL => OpenGL::supported(),
            Self::VULKAN => Vulkan::supported(),
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
            Self::VULKAN => Vulkan::direct_context(),
            Self::GL => OpenGL::direct_context(),
            Self::CPU => None
        }
    }
}
