#![allow(clippy::upper_case_acronyms)]
use skia_safe::{ImageInfo, Surface, surfaces};
use serde_json::Value;

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
    pub fn status() -> Value { serde_json::json!({
        "renderer": "CPU",
        "api": Value::Null,
        "device": "CPU-based renderer (compiled without GPU support)",
        "error": Value::Null,
    })}
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

#[allow(dead_code)]
impl RenderingEngine{
    pub fn selectable(&self) -> bool {
        match self {
            Self::GPU => Engine::supported(),
            Self::CPU => true
        }
    }

    pub fn get_surface(&self, image_info: &ImageInfo) -> Option<Surface> {
        match self {
            Self::GPU => Engine::surface(image_info),
            Self::CPU => surfaces::raster(image_info, None, None)
        }
    }

    pub fn status(&self) -> serde_json::Value {
        let mut status = Engine::status();
        if let Self::CPU = self{
            if Engine::supported(){
                status["renderer"] = Value::String("CPU".to_string());
                status["device"] = Value::String("CPU-based renderer (GPU manually disabled)".to_string())
            }
        }
        status
    }

    pub fn lacks_gpu_support(&self) -> Option<String> {
        match Engine::supported(){
            true => None,
            false => {
                let mut msg = vec!["No windowing support".to_string()];
                if let Some(Value::String(error)) = Engine::status().get("error"){
                    msg.push(error.to_string());
                }
                Some(msg.join(": "))
            }
        }   
    }
}
