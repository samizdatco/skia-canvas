#![allow(clippy::unnecessary_wraps)]
use std::sync::{Mutex};
use neon::prelude::*;
use once_cell::sync::Lazy;

mod canvas;
mod context;
mod path;
mod image;
mod filter;
mod gradient;
mod pattern;
mod texture;
mod typography;
mod utils;
mod gpu;
#[cfg(feature = "window")]
mod gui;

use context::api as ctx;
use typography::FontLibrary;

pub static FONT_LIBRARY: Lazy<Mutex<FontLibrary>> = Lazy::new(|| FontLibrary::shared() );

#[neon::main]
fn main(mut cx: ModuleContext) -> NeonResult<()> {

  // -- Image -------------------------------------------------------------------------------------

  cx.export_function("Image_new", image::new)?;
  cx.export_function("Image_get_src", image::get_src)?;
  cx.export_function("Image_set_src", image::set_src)?;
  cx.export_function("Image_set_data", image::set_data)?;
  cx.export_function("Image_get_width", image::get_width)?;
  cx.export_function("Image_get_height", image::get_height)?;
  cx.export_function("Image_get_complete", image::get_complete)?;

  // -- Path2D ------------------------------------------------------------------------------------

  cx.export_function("Path2D_new", path::new)?;
  cx.export_function("Path2D_from_path", path::from_path)?;
  cx.export_function("Path2D_from_svg", path::from_svg)?;
  cx.export_function("Path2D_addPath", path::addPath)?;
  cx.export_function("Path2D_closePath", path::closePath)?;
  cx.export_function("Path2D_moveTo", path::moveTo)?;
  cx.export_function("Path2D_lineTo", path::lineTo)?;
  cx.export_function("Path2D_bezierCurveTo", path::bezierCurveTo)?;
  cx.export_function("Path2D_quadraticCurveTo", path::quadraticCurveTo)?;
  cx.export_function("Path2D_conicCurveTo", path::conicCurveTo)?;
  cx.export_function("Path2D_arc", path::arc)?;
  cx.export_function("Path2D_arcTo", path::arcTo)?;
  cx.export_function("Path2D_ellipse", path::ellipse)?;
  cx.export_function("Path2D_rect", path::rect)?;
  cx.export_function("Path2D_roundRect", path::roundRect)?;
  cx.export_function("Path2D_op", path::op)?;
  cx.export_function("Path2D_interpolate", path::interpolate)?;
  cx.export_function("Path2D_simplify", path::simplify)?;
  cx.export_function("Path2D_unwind", path::unwind)?;
  cx.export_function("Path2D_round", path::round)?;
  cx.export_function("Path2D_trim", path::trim)?;
  cx.export_function("Path2D_jitter", path::jitter)?;
  cx.export_function("Path2D_offset", path::offset)?;
  cx.export_function("Path2D_transform", path::transform)?;
  cx.export_function("Path2D_bounds", path::bounds)?;
  cx.export_function("Path2D_contains", path::contains)?;
  cx.export_function("Path2D_edges", path::edges)?;
  cx.export_function("Path2D_get_d", path::get_d)?;
  cx.export_function("Path2D_set_d", path::set_d)?;

  // -- CanvasGradient ----------------------------------------------------------------------------

  cx.export_function("CanvasGradient_linear", gradient::linear)?;
  cx.export_function("CanvasGradient_radial", gradient::radial)?;
  cx.export_function("CanvasGradient_conic", gradient::conic)?;
  cx.export_function("CanvasGradient_addColorStop", gradient::addColorStop)?;
  cx.export_function("CanvasGradient_repr", gradient::repr)?;

  // -- CanvasPattern -----------------------------------------------------------------------------

  cx.export_function("CanvasPattern_from_image", pattern::from_image)?;
  cx.export_function("CanvasPattern_from_canvas", pattern::from_canvas)?;
  cx.export_function("CanvasPattern_setTransform", pattern::setTransform)?;
  cx.export_function("CanvasPattern_repr", pattern::repr)?;

  // -- CanvasTexture -----------------------------------------------------------------------------

  cx.export_function("CanvasTexture_new", texture::new)?;
  cx.export_function("CanvasTexture_repr", texture::repr)?;

  // -- FontLibrary -------------------------------------------------------------------------------

  cx.export_function("FontLibrary_get_families", typography::get_families)?;
  cx.export_function("FontLibrary_has", typography::has)?;
  cx.export_function("FontLibrary_family", typography::family)?;
  cx.export_function("FontLibrary_addFamily", typography::addFamily)?;
  cx.export_function("FontLibrary_reset", typography::reset)?;

  // -- Canvas ------------------------------------------------------------------------------------

  cx.export_function("Canvas_new", canvas::new)?;

  cx.export_function("Canvas_get_engine", canvas::get_engine)?;
  cx.export_function("Canvas_set_engine", canvas::set_engine)?;

  cx.export_function("Canvas_get_width", canvas::get_width)?;
  cx.export_function("Canvas_set_width", canvas::set_width)?;
  cx.export_function("Canvas_get_height", canvas::get_height)?;
  cx.export_function("Canvas_set_height", canvas::set_height)?;

  cx.export_function("Canvas_get_async", canvas::get_async)?;
  cx.export_function("Canvas_set_async", canvas::set_async)?;
  cx.export_function("Canvas_save", canvas::save)?;
  cx.export_function("Canvas_saveSync", canvas::saveSync)?;
  cx.export_function("Canvas_toBuffer", canvas::toBuffer)?;
  cx.export_function("Canvas_toBufferSync", canvas::toBufferSync)?;

  // -- Context -----------------------------------------------------------------------------------

  cx.export_function("CanvasRenderingContext2D_new", ctx::new)?;
  cx.export_function("CanvasRenderingContext2D_resetSize", ctx::resetSize)?;
  cx.export_function("CanvasRenderingContext2D_get_size", ctx::get_size)?;
  cx.export_function("CanvasRenderingContext2D_set_size", ctx::set_size)?;
  cx.export_function("CanvasRenderingContext2D_reset", ctx::reset)?;

  // grid state
  cx.export_function("CanvasRenderingContext2D_save", ctx::save)?;
  cx.export_function("CanvasRenderingContext2D_restore", ctx::restore)?;
  cx.export_function("CanvasRenderingContext2D_transform", ctx::transform)?;
  cx.export_function("CanvasRenderingContext2D_translate", ctx::translate)?;
  cx.export_function("CanvasRenderingContext2D_scale", ctx::scale)?;
  cx.export_function("CanvasRenderingContext2D_rotate", ctx::rotate)?;
  cx.export_function("CanvasRenderingContext2D_resetTransform", ctx::resetTransform)?;
  cx.export_function("CanvasRenderingContext2D_get_currentTransform", ctx::get_currentTransform)?;
  cx.export_function("CanvasRenderingContext2D_set_currentTransform", ctx::set_currentTransform)?;
  cx.export_function("CanvasRenderingContext2D_createProjection", ctx::createProjection)?;

  // bézier paths
  cx.export_function("CanvasRenderingContext2D_beginPath", ctx::beginPath)?;
  cx.export_function("CanvasRenderingContext2D_rect", ctx::rect)?;
  cx.export_function("CanvasRenderingContext2D_roundRect", ctx::roundRect)?;
  cx.export_function("CanvasRenderingContext2D_arc", ctx::arc)?;
  cx.export_function("CanvasRenderingContext2D_ellipse", ctx::ellipse)?;
  cx.export_function("CanvasRenderingContext2D_moveTo", ctx::moveTo)?;
  cx.export_function("CanvasRenderingContext2D_lineTo", ctx::lineTo)?;
  cx.export_function("CanvasRenderingContext2D_arcTo", ctx::arcTo)?;
  cx.export_function("CanvasRenderingContext2D_bezierCurveTo", ctx::bezierCurveTo)?;
  cx.export_function("CanvasRenderingContext2D_quadraticCurveTo", ctx::quadraticCurveTo)?;
  cx.export_function("CanvasRenderingContext2D_conicCurveTo", ctx::conicCurveTo)?;
  cx.export_function("CanvasRenderingContext2D_closePath", ctx::closePath)?;
  cx.export_function("CanvasRenderingContext2D_isPointInPath", ctx::isPointInPath)?;
  cx.export_function("CanvasRenderingContext2D_isPointInStroke", ctx::isPointInStroke)?;
  cx.export_function("CanvasRenderingContext2D_clip", ctx::clip)?;

  // fill & stroke
  cx.export_function("CanvasRenderingContext2D_fill", ctx::fill)?;
  cx.export_function("CanvasRenderingContext2D_stroke", ctx::stroke)?;
  cx.export_function("CanvasRenderingContext2D_fillRect", ctx::fillRect)?;
  cx.export_function("CanvasRenderingContext2D_strokeRect", ctx::strokeRect)?;
  cx.export_function("CanvasRenderingContext2D_clearRect", ctx::clearRect)?;
  cx.export_function("CanvasRenderingContext2D_get_fillStyle", ctx::get_fillStyle)?;
  cx.export_function("CanvasRenderingContext2D_set_fillStyle", ctx::set_fillStyle)?;
  cx.export_function("CanvasRenderingContext2D_get_strokeStyle", ctx::get_strokeStyle)?;
  cx.export_function("CanvasRenderingContext2D_set_strokeStyle", ctx::set_strokeStyle)?;

  // line style
  cx.export_function("CanvasRenderingContext2D_getLineDash", ctx::getLineDash)?;
  cx.export_function("CanvasRenderingContext2D_setLineDash", ctx::setLineDash)?;
  cx.export_function("CanvasRenderingContext2D_get_lineCap", ctx::get_lineCap)?;
  cx.export_function("CanvasRenderingContext2D_set_lineCap", ctx::set_lineCap)?;
  cx.export_function("CanvasRenderingContext2D_get_lineDashFit", ctx::get_lineDashFit)?;
  cx.export_function("CanvasRenderingContext2D_set_lineDashFit", ctx::set_lineDashFit)?;
  cx.export_function("CanvasRenderingContext2D_get_lineDashMarker", ctx::get_lineDashMarker)?;
  cx.export_function("CanvasRenderingContext2D_set_lineDashMarker", ctx::set_lineDashMarker)?;
  cx.export_function("CanvasRenderingContext2D_get_lineDashOffset", ctx::get_lineDashOffset)?;
  cx.export_function("CanvasRenderingContext2D_set_lineDashOffset", ctx::set_lineDashOffset)?;
  cx.export_function("CanvasRenderingContext2D_get_lineJoin", ctx::get_lineJoin)?;
  cx.export_function("CanvasRenderingContext2D_set_lineJoin", ctx::set_lineJoin)?;
  cx.export_function("CanvasRenderingContext2D_get_lineWidth", ctx::get_lineWidth)?;
  cx.export_function("CanvasRenderingContext2D_set_lineWidth", ctx::set_lineWidth)?;
  cx.export_function("CanvasRenderingContext2D_get_miterLimit", ctx::get_miterLimit)?;
  cx.export_function("CanvasRenderingContext2D_set_miterLimit", ctx::set_miterLimit)?;

  // imagery
  cx.export_function("CanvasRenderingContext2D_drawImage", ctx::drawImage)?;
  cx.export_function("CanvasRenderingContext2D_drawCanvas", ctx::drawCanvas)?;
  cx.export_function("CanvasRenderingContext2D_getImageData", ctx::getImageData)?;
  cx.export_function("CanvasRenderingContext2D_putImageData", ctx::putImageData)?;
  cx.export_function("CanvasRenderingContext2D_get_imageSmoothingEnabled", ctx::get_imageSmoothingEnabled)?;
  cx.export_function("CanvasRenderingContext2D_set_imageSmoothingEnabled", ctx::set_imageSmoothingEnabled)?;
  cx.export_function("CanvasRenderingContext2D_get_imageSmoothingQuality", ctx::get_imageSmoothingQuality)?;
  cx.export_function("CanvasRenderingContext2D_set_imageSmoothingQuality", ctx::set_imageSmoothingQuality)?;

  // typography
  cx.export_function("CanvasRenderingContext2D_fillText", ctx::fillText)?;
  cx.export_function("CanvasRenderingContext2D_strokeText", ctx::strokeText)?;
  cx.export_function("CanvasRenderingContext2D_measureText", ctx::measureText)?;
  cx.export_function("CanvasRenderingContext2D_outlineText", ctx::outlineText)?;
  cx.export_function("CanvasRenderingContext2D_get_font", ctx::get_font)?;
  cx.export_function("CanvasRenderingContext2D_set_font", ctx::set_font)?;
  cx.export_function("CanvasRenderingContext2D_get_textAlign", ctx::get_textAlign)?;
  cx.export_function("CanvasRenderingContext2D_set_textAlign", ctx::set_textAlign)?;
  cx.export_function("CanvasRenderingContext2D_get_textBaseline", ctx::get_textBaseline)?;
  cx.export_function("CanvasRenderingContext2D_set_textBaseline", ctx::set_textBaseline)?;
  cx.export_function("CanvasRenderingContext2D_get_direction", ctx::get_direction)?;
  cx.export_function("CanvasRenderingContext2D_set_direction", ctx::set_direction)?;
  cx.export_function("CanvasRenderingContext2D_get_fontVariant", ctx::get_fontVariant)?;
  cx.export_function("CanvasRenderingContext2D_set_fontVariant", ctx::set_fontVariant)?;
  cx.export_function("CanvasRenderingContext2D_get_textTracking", ctx::get_textTracking)?;
  cx.export_function("CanvasRenderingContext2D_set_textTracking", ctx::set_textTracking)?;
  cx.export_function("CanvasRenderingContext2D_get_textWrap", ctx::get_textWrap)?;
  cx.export_function("CanvasRenderingContext2D_set_textWrap", ctx::set_textWrap)?;

  // effects
  cx.export_function("CanvasRenderingContext2D_get_globalAlpha", ctx::get_globalAlpha)?;
  cx.export_function("CanvasRenderingContext2D_set_globalAlpha", ctx::set_globalAlpha)?;
  cx.export_function("CanvasRenderingContext2D_get_globalCompositeOperation", ctx::get_globalCompositeOperation)?;
  cx.export_function("CanvasRenderingContext2D_set_globalCompositeOperation", ctx::set_globalCompositeOperation)?;
  cx.export_function("CanvasRenderingContext2D_get_filter", ctx::get_filter)?;
  cx.export_function("CanvasRenderingContext2D_set_filter", ctx::set_filter)?;
  cx.export_function("CanvasRenderingContext2D_get_shadowBlur", ctx::get_shadowBlur)?;
  cx.export_function("CanvasRenderingContext2D_set_shadowBlur", ctx::set_shadowBlur)?;
  cx.export_function("CanvasRenderingContext2D_get_shadowColor", ctx::get_shadowColor)?;
  cx.export_function("CanvasRenderingContext2D_set_shadowColor", ctx::set_shadowColor)?;
  cx.export_function("CanvasRenderingContext2D_get_shadowOffsetX", ctx::get_shadowOffsetX)?;
  cx.export_function("CanvasRenderingContext2D_get_shadowOffsetY", ctx::get_shadowOffsetY)?;
  cx.export_function("CanvasRenderingContext2D_set_shadowOffsetX", ctx::set_shadowOffsetX)?;
  cx.export_function("CanvasRenderingContext2D_set_shadowOffsetY", ctx::set_shadowOffsetY)?;

  // -- Window -----------------------------------------------------------------------------------

  #[cfg(feature = "window")] {
    cx.export_function("App_launch", gui::launch)?;
    cx.export_function("App_quit", gui::quit)?;
    cx.export_function("App_closeWindow", gui::close)?;
    cx.export_function("App_openWindow", gui::open)?;
    cx.export_function("App_setRate", gui::set_rate)?;
  }

  Ok(())
}
