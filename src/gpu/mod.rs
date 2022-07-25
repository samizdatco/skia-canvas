use skia_safe::{ImageInfo, Surface};

#[cfg(feature = "metal")]
mod metal;
#[cfg(feature = "metal")]
use crate::gpu::metal::Engine;

#[cfg(feature = "vulkan")]
mod vulkan;
#[cfg(feature = "vulkan")]
use crate::gpu::vulkan::Engine;

#[cfg(not(any(feature = "vulkan", feature = "metal")))]
struct Engine { }
#[cfg(not(any(feature = "vulkan", feature = "metal")))]
impl Engine {
    pub fn supported() -> bool { false }
    pub fn surface(_: &ImageInfo) -> Option<Surface> { None }
}

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
        match self {
            Self::GPU => Engine::surface(image_info),
            Self::CPU => Surface::new_raster(image_info, None, None)
        }
    }
}
