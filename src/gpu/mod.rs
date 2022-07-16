use neon::context::{Context, FunctionContext};
use neon::result::JsResult;
use neon::types::JsString;
use skia_safe::gpu::{DirectContext, SurfaceOrigin};
use skia_safe::{Budgeted, ImageInfo, Surface};
use crate::gpu::gl::{get_gl_context, gl_supported};
use crate::gpu::vulkan::{get_vulkan_context, vulkan_supported};

mod vulkan;
mod gl;

pub fn gpu_support(mut cx: FunctionContext) -> JsResult<JsString> {
    Ok(if vulkan_supported() {
        cx.string("vulkan")
    } else if gl_supported() {
        cx.string("gl")
    } else {
        cx.string("none")
    })
}

fn get_direct_context() -> Option<DirectContext> {
    if vulkan_supported() {
        Some(get_vulkan_context())
    } else if gl_supported() {
        Some(get_gl_context())
    } else {
        None
    }
}

pub fn get_surface(image_info: &ImageInfo) -> Option<Surface> {
    if let Some(mut context) = get_direct_context() {
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
