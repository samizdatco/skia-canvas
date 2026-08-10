use neon::prelude::*;
use skia_safe::Point;

use super::*;

//
// Filters
//

use crate::filter::{FilterSpec, FilterQuality};

pub fn filter_arg(cx: &mut FunctionContext, idx: usize) -> NeonResult<(String, Vec<FilterSpec>)> {
  let arg = cx.argument::<JsObject>(idx)?;
  let canonical = string_for_key(cx, &arg, "canonical")?;

  let obj:Handle<JsObject> = arg.get(cx, "filters")?;
  let keys = obj.get_own_property_names(cx)?.to_vec(cx)?;
  let mut filters = vec![];
  for (name, key) in strings_in(cx, &keys).iter().zip(keys) {
    match name.as_str() {
      "drop-shadow" => {
        let values = obj.get::<JsArray, _, _>(cx, key)?;
        let nums = values.to_vec(cx)?;
        let dims = floats_in(cx, &nums);
        let color_str = values.get::<JsString, _, _>(cx, 3)?.value(cx);
        if let Some(color) = css_to_color4f(&color_str) {
          filters.push(FilterSpec::Shadow{
            offset: Point::new(dims[0], dims[1]), blur: dims[2], color
          });
        }
      },
      _ => {
        let value = obj.get::<JsNumber, _, _>(cx, key)?.value(cx) as f32;
        filters.push(FilterSpec::Plain{
          name:name.to_string(), value
        })
      }
    }
  }
  Ok( (canonical, filters) )
}

pub fn to_filter_quality(mode_name:&str) -> Option<FilterQuality>{
  let mode = match mode_name.to_lowercase().as_str(){
    "low" => FilterQuality::Low,
    "medium" => FilterQuality::Medium,
    "high" => FilterQuality::High,
    _ => return None
  };
  Some(mode)
}

pub fn from_filter_quality(mode:FilterQuality) -> String{
  match mode{
    FilterQuality::Low => "low",
    FilterQuality::Medium => "medium",
    FilterQuality::High => "high",
    _ => "low"
  }.to_string()
}

//
// CanvasPattern
//

pub fn repetition_arg<'a>(cx: &mut FunctionContext<'a>, idx: usize) -> NeonResult<(TileMode, TileMode)>{
  let repetition = if cx.len() > idx && cx.argument::<JsValue>(idx)?.is_a::<JsNull, _>(cx){
    "".to_string() // null is a valid synonym for "repeat" (as is "")
  }else{
    string_arg(cx, idx, "repetition")?
  };

  match to_repeat_mode(&repetition){
    Some(mode) => Ok(mode),
    None => cx.throw_type_error("Expected `repetition` to be \"repeat\", \"repeat-x\", \"repeat-y\", or \"no-repeat\"")
  }
}

//
// Skia Enums
//

use skia_safe::{TileMode, TileMode::{Decal, Repeat}};
pub fn to_repeat_mode(repeat:&str) -> Option<(TileMode, TileMode)> {
  let mode = match repeat.to_lowercase().as_str() {
    "repeat" | "" => (Repeat, Repeat),
    "repeat-x" => (Repeat, Decal),
    "repeat-y" => (Decal, Repeat),
    "no-repeat" => (Decal, Decal),
    _ => return None
  };
  Some(mode)
}

use skia_safe::{PaintCap};
pub fn to_stroke_cap(mode_name:&str) -> Option<PaintCap>{
  let mode = match mode_name.to_lowercase().as_str(){
    "butt" => PaintCap::Butt,
    "round" => PaintCap::Round,
    "square" => PaintCap::Square,
        _ => return None
  };
  Some(mode)
}

pub fn from_stroke_cap(mode:PaintCap) -> String{
  match mode{
    PaintCap::Butt => "butt",
    PaintCap::Round => "round",
    PaintCap::Square => "square",
  }.to_string()
}

use skia_safe::{PaintJoin};
pub fn to_stroke_join(mode_name:&str) -> Option<PaintJoin>{
  let mode = match mode_name.to_lowercase().as_str(){
    "miter" => PaintJoin::Miter,
    "round" => PaintJoin::Round,
    "bevel" => PaintJoin::Bevel,
    _ => return None
  };
  Some(mode)
}

pub fn from_stroke_join(mode:PaintJoin) -> String{
  match mode{
    PaintJoin::Miter => "miter",
    PaintJoin::Round => "round",
    PaintJoin::Bevel => "bevel",
  }.to_string()
}


use skia_safe::{BlendMode};
pub fn to_blend_mode(mode_name:&str) -> Option<BlendMode>{
  let mode = match mode_name.to_lowercase().as_str(){
    "source-over" => BlendMode::SrcOver,
    "destination-over" => BlendMode::DstOver,
    "copy" => BlendMode::Src,
    "destination" => BlendMode::Dst,
    "clear" => BlendMode::Clear,
    "source-in" => BlendMode::SrcIn,
    "destination-in" => BlendMode::DstIn,
    "source-out" => BlendMode::SrcOut,
    "destination-out" => BlendMode::DstOut,
    "source-atop" => BlendMode::SrcATop,
    "destination-atop" => BlendMode::DstATop,
    "xor" => BlendMode::Xor,
    "lighter" => BlendMode::Plus,
    "multiply" => BlendMode::Multiply,
    "screen" => BlendMode::Screen,
    "overlay" => BlendMode::Overlay,
    "darken" => BlendMode::Darken,
    "lighten" => BlendMode::Lighten,
    "color-dodge" => BlendMode::ColorDodge,
    "color-burn" => BlendMode::ColorBurn,
    "hard-light" => BlendMode::HardLight,
    "soft-light" => BlendMode::SoftLight,
    "difference" => BlendMode::Difference,
    "exclusion" => BlendMode::Exclusion,
    "hue" => BlendMode::Hue,
    "saturation" => BlendMode::Saturation,
    "color" => BlendMode::Color,
    "luminosity" => BlendMode::Luminosity,
    _ => return None
  };
  Some(mode)
}

pub fn from_blend_mode(mode:BlendMode) -> String{
  match mode{
    BlendMode::SrcOver => "source-over",
    BlendMode::DstOver => "destination-over",
    BlendMode::Src => "copy",
    BlendMode::Dst => "destination",
    BlendMode::Clear => "clear",
    BlendMode::SrcIn => "source-in",
    BlendMode::DstIn => "destination-in",
    BlendMode::SrcOut => "source-out",
    BlendMode::DstOut => "destination-out",
    BlendMode::SrcATop => "source-atop",
    BlendMode::DstATop => "destination-atop",
    BlendMode::Xor => "xor",
    BlendMode::Plus => "lighter",
    BlendMode::Multiply => "multiply",
    BlendMode::Screen => "screen",
    BlendMode::Overlay => "overlay",
    BlendMode::Darken => "darken",
    BlendMode::Lighten => "lighten",
    BlendMode::ColorDodge => "color-dodge",
    BlendMode::ColorBurn => "color-burn",
    BlendMode::HardLight => "hard-light",
    BlendMode::SoftLight => "soft-light",
    BlendMode::Difference => "difference",
    BlendMode::Exclusion => "exclusion",
    BlendMode::Hue => "hue",
    BlendMode::Saturation => "saturation",
    BlendMode::Color => "color",
    BlendMode::Luminosity => "luminosity",
    _ => "source-over"
  }.to_string()
}

use crate::gfx::RenderingEngine;
pub fn to_engine(engine_name:&str) -> Option<RenderingEngine>{
  let mode = match engine_name.to_lowercase().as_str(){
    "gpu" => RenderingEngine::GPU,
    "cpu" => RenderingEngine::CPU,
    _ => return None
  };
  Some(mode)
}

pub fn from_engine(engine:RenderingEngine) -> String{
  match engine{
    RenderingEngine::GPU => "gpu",
    RenderingEngine::CPU => "cpu",
  }.to_string()
}

// pub fn blend_mode_arg(cx: &mut FunctionContext, idx: usize, attr: &str) -> NeonResult<BlendMode>{
//   let mode_name = string_arg(cx, idx, attr)?;
//   match to_blend_mode(&mode_name){
//     Some(blend_mode) => Ok(blend_mode),
//     None => cx.throw_error("blendMode must be SrcOver, DstOver, Src, Dst, Clear, SrcIn, DstIn, \
//                             SrcOut, DstOut, SrcATop, DstATop, Xor, Plus, Multiply, Screen, Overlay, \
//                             Darken, Lighten, ColorDodge, ColorBurn, HardLight, SoftLight, Difference, \
//                             Exclusion, Hue, Saturation, Color, Luminosity, or Modulate")
//   }
// }
