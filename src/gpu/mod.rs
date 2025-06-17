#![allow(clippy::upper_case_acronyms)]
use skia_safe::{ImageInfo, Image, Rect, Matrix, Color, Surface, surfaces};
use serde_json::Value;
use crate::context::page::Page;

#[cfg(feature = "metal")]
mod metal;
#[cfg(feature = "metal")]
use crate::gpu::metal::MetalEngine as Engine;
#[cfg(all(feature = "metal", feature = "window"))]
pub use crate::gpu::metal::MetalRenderer as Renderer;


#[cfg(feature = "vulkan")]
mod vulkan;
#[cfg(feature = "vulkan")]
use crate::gpu::vulkan::engine::VulkanEngine as Engine;
#[cfg(all(feature = "vulkan", feature = "window"))]
pub use crate::gpu::vulkan::renderer::VulkanRenderer as Renderer;

#[cfg(not(any(feature = "vulkan", feature = "metal")))]
struct Engine { }
#[cfg(not(any(feature = "vulkan", feature = "metal")))]
impl Engine {
    pub fn supported() -> bool { false }
    pub fn with_surface<T, F>(_: &ImageInfo, _:Option<usize>, _:F)  -> Result<T, String>
        where F:FnOnce(&mut Surface) -> Result<T, String>
    {
        Err("Compiled without GPU support".to_string())
    }
    pub fn status() -> Value { serde_json::json!({
        "renderer": "CPU",
        "api": Value::Null,
        "device": "CPU-based renderer (compiled without GPU support)",
        "error": Value::Null,
    })}
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

#[allow(dead_code)]
impl RenderingEngine{
    pub fn selectable(&self) -> bool {
        match self {
            Self::GPU => Engine::supported(),
            Self::CPU => true
        }
    }

    pub fn with_surface<T,F>(&self, image_info: &ImageInfo, msaa:Option<usize>, f:F) -> Result<T, String>
        where F:FnOnce(&mut Surface) -> Result<T, String>
    {
        match self {
            Self::GPU => Engine::with_surface(image_info, msaa, f),
            Self::CPU => surfaces::raster(image_info, None, None)
                .ok_or(format!("Could not allocate new {}×{} bitmap", image_info.width(), image_info.height()))
                .and_then(|mut surface|f(&mut surface))
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

pub struct RenderCache {
    image: Option<Image>,
    content: Rect,
    page: Page,
    matte: Color,
    dpr: f32,
    state: RenderState,
}

impl Default for RenderCache{
    fn default() -> Self {
        Self{image:None, content:Rect::new_empty(), page:Page::default(), dpr:0.0, matte:Color::TRANSPARENT, state:RenderState::Clean}
    }
}

impl RenderCache{
    pub fn validate(&mut self, page:&Page, matte:Color, dpr:f32, clip:Rect) -> Option<(&Image, &Rect, Rect)>{
        if self.state == RenderState::Dirty{
            self.clear();
        }

        let is_valid =
            self.page.id == page.id &&
            self.page.rev == page.rev &&
            self.matte == matte &&
            self.dpr == dpr;

        match is_valid{
            true => self.image.as_ref().map(|img| {
                let (dst, _) = Matrix::scale((dpr, dpr)).map_rect(clip);
                (img, &self.content, dst)
            }),
            false => None
        }
    }

    pub fn depth(&self) -> usize {
        self.page.layers.len()
    }

    pub fn update(&mut self, image:Image, page:&Page, matte:Color, dpr:f32, content:Rect){
        if self.state==RenderState::Resizing{
            // mark the framebuffer as needing a full redraw and skip updating cached image during resize
            self.state = RenderState::Dirty;
        }else{
            let state = RenderState::Clean;
            let (content, _) = skia_safe::Matrix::scale((dpr, dpr)).map_rect(content);
            *self = Self{image: Some(image), page:page.clone(), matte, dpr, content, state};
        }
    }

    pub fn clear(&mut self){
        *self = Self::default();
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum RenderState{
    Clean,
    Dirty,
    Resizing
}
