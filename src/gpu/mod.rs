use skia_safe::{ImageInfo, Surface};

#[cfg(feature = "metal")]
mod metal;
#[cfg(feature = "metal")]
use crate::gpu::metal::MetalEngine as Engine;
#[cfg(all(feature = "metal", feature = "window"))]
pub use crate::gpu::metal::MetalRenderer as Renderer;


#[cfg(feature = "vulkan")]
mod vulkan;
#[cfg(feature = "vulkan")]
use crate::gpu::vulkan::VulkanEngine as Engine;
#[cfg(all(feature = "vulkan", feature = "window"))]
pub use crate::gpu::vulkan::VulkanRenderer as Renderer;

#[cfg(not(any(feature = "vulkan", feature = "metal")))]
struct Engine { }
#[cfg(not(any(feature = "vulkan", feature = "metal")))]
impl Engine {
    pub fn supported() -> bool { false }
    pub fn surface(_: &ImageInfo) -> Option<Surface> { None }
}

#[cfg(feature = "metal")]
pub use crate::gpu::metal::autoreleasepool as runloop;
#[cfg(not(feature = "metal"))]
#[allow(dead_code)]
pub fn runloop<T, F: FnOnce() -> T>(f: F) -> T { f() }

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
