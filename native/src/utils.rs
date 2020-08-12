#![allow(dead_code)]
#![allow(unused_imports)]
use std::cmp;
use std::f32::consts::PI;
use core::ops::Range;
use neon::prelude::*;
use neon::result::Throw;
use neon::object::This;
use skia_safe::{Path, Matrix, Point, Color, Color4f, RGB};
use skia_safe::{Font, FontMetrics, font_style::{FontStyle, Weight, Width, Slant}};
use css_color::Rgba;

use crate::path::{JsPath2D};

//
// meta-helpers
//

fn arg_num(o:usize) -> String{
  let n = (o + 1) as i32; // we're working with zero-bounded idxs
  let ords = ["st","nd","rd"];
  let slot = ((n+90)%100-10)%10 - 1;
  let suffix = if slot >= 0 && slot < 3 { ords[slot as usize] } else { "th" };
  format!("{}{}", n, suffix)
}

pub fn argv<'a>() -> Vec<Handle<'a, JsValue>>{
  let list:Vec<Handle<JsValue>> = Vec::new();
  list
}

pub fn clamp(val: f32, min:f64, max:f64) -> f32{
  let min = min as f32;
  let max = max as f32;
  if val < min { min } else if val > max { max } else { val }
}

pub fn almost_equal(a: f32, b: f32) -> bool{
  (a-b).abs() < 0.00001
}

pub fn to_degrees(radians: f32) -> f32{
  radians / PI * 180.0
}

pub fn to_radians(degrees: f32) -> f32{
  degrees / 180.0 * PI
}

pub fn symbol<'a, T: This>(cx: &mut CallContext<'a, T>, symbol_name: &str) -> JsResult<'a, JsValue> {
  let global = cx.global();
  let symbol_ctor = global
      .get(cx, "Symbol")?
      .downcast::<JsObject>()
      .or_throw(cx)?
      .get(cx, "for")?
      .downcast::<JsFunction>()
      .or_throw(cx)?;

  let symbol_label = cx.string(symbol_name);
  let sym = symbol_ctor.call(cx, global, vec![symbol_label])?;
  Ok(sym)
}

//
// strings
//


pub fn strings_in(vals: &[Handle<JsValue>]) -> Vec<String>{
  vals.iter()
      .map(|js_val| js_val.downcast::<JsString>())
      .filter( |r| r.is_ok() )
      .map( |num| num.as_ref().unwrap().value() )
      .collect()
}

pub fn strings_at_key<T: This>(cx: &mut CallContext<'_, T>, obj: &Handle<JsObject>, attr:&str) -> Result<Vec<String>, Throw>{
  let array = obj.get(cx, attr)?.downcast::<JsArray>().or_throw(cx)?.to_vec(cx)?;
  Ok(strings_in(&array))
}

pub fn string_for_key<T: This>(cx: &mut CallContext<'_, T>, obj: &Handle<JsObject>, attr:&str) -> Result<String, Throw>{
  let key = cx.string(attr);
  match obj.get(cx, key)?.downcast::<JsString>(){
    Ok(s) => Ok(s.value()),
    Err(_e) => cx.throw_error(format!("Exptected a string for \"{}\"", attr))
  }
}

pub fn opt_string_arg<T: This>(cx: &mut CallContext<'_, T>, idx: usize) -> Option<String>{
  match cx.argument_opt(idx as i32) {
    Some(arg) => match arg.downcast::<JsString>() {
      Ok(v) => Some(v.value()),
      Err(_e) => None
    },
    None => None
  }
}

pub fn string_arg_or<T: This>(cx: &mut CallContext<'_, T>, idx: usize, default:&str) -> String{
  match opt_string_arg(cx, idx){
    Some(v) => v,
    None => String::from(default)
  }
}


pub fn string_arg<'a, T: This>(cx: &mut CallContext<'a, T>, idx: usize, attr:&str) -> Result<String, Throw> {
  let exists = cx.len() > idx as i32;
  match opt_string_arg(cx, idx){
    Some(v) => Ok(v),
    None => cx.throw_type_error(
      if exists { format!("{} must be a string", attr) }
      else { format!("missing argument: expected a string for {} ({} arg)", attr, arg_num(idx)) }
    )
  }
}

//
// bools
//

pub fn opt_bool_arg<T: This>(cx: &mut CallContext<'_, T>, idx: usize) -> Option<bool>{
  match cx.argument_opt(idx as i32) {
    Some(arg) => match arg.downcast::<JsBoolean>() {
      Ok(v) => Some(v.value()),
      Err(_e) => None
    },
    None => None
  }
}

pub fn bool_arg_or<T: This>(cx: &mut CallContext<'_, T>, idx: usize, default:bool) -> bool{
  match opt_bool_arg(cx, idx){
    Some(v) => v,
    None => default
  }
}

pub fn bool_arg<T: This>(cx: &mut CallContext<'_, T>, idx: usize, attr:&str) -> Result<bool, Throw>{
  let exists = cx.len() > idx as i32;
  match opt_bool_arg(cx, idx){
    Some(v) => Ok(v),
    None => cx.throw_type_error(
      if exists { format!("{} must be a boolean", attr) }
      else { format!("missing argument: expected a boolean for {} (as {} arg)", attr, arg_num(idx)) }
    )
  }
}

//
// floats
//


pub fn float_for_key<T: This>(cx: &mut CallContext<'_, T>, obj: &Handle<JsObject>, attr:&str) -> Result<f32, Throw>{
  let key = cx.string(attr);
  match obj.get(cx, key)?.downcast::<JsNumber>(){
    Ok(num) => Ok(num.value() as f32),
    Err(_e) => cx.throw_error(format!("Exptected a numerical value for \"{}\"", attr))
  }
}

pub fn floats_in(vals: &[Handle<JsValue>]) -> Vec<f32>{
  vals.iter()
      .map(|js_val| js_val.downcast::<JsNumber>())
      .filter( |r| r.is_ok() )
      .map( |num| num.as_ref().unwrap().value() as f32 )
      .collect()
}

pub fn opt_float_arg<T: This>(cx: &mut CallContext<'_, T>, idx: usize) -> Option<f32>{
  match cx.argument_opt(idx as i32) {
    Some(arg) => match arg.downcast::<JsNumber>() {
      Ok(v) => Some(v.value() as f32),
      Err(_e) => None
    },
    None => None
  }
}

pub fn float_arg_or<T: This>(cx: &mut CallContext<'_, T>, idx: usize, default:f64) -> f32{
  match opt_float_arg(cx, idx){
    Some(v) => v,
    None => default as f32
  }
}

pub fn float_arg<T: This>(cx: &mut CallContext<'_, T>, idx: usize, attr:&str) -> Result<f32, Throw>{
  let exists = cx.len() > idx as i32;
  match opt_float_arg(cx, idx){
    Some(v) => Ok(v),
    None => cx.throw_type_error(
      if exists { format!("{} must be a number", attr) }
      else { format!("missing argument: expected a number for {} as {} arg", attr, arg_num(idx)) }
    )
  }
}

pub fn floats_to_array<'a, T: This>(cx: &mut CallContext<'a, T>, nums: &[f32]) -> JsResult<'a, JsValue> {
  let array = JsArray::new(cx, nums.len() as u32);
  for (i, val) in nums.iter().enumerate() {
    let num = cx.number(*val);
    array.set(cx, i as u32, num)?;
  }
  Ok(array.upcast())
}

//
// float spreads
//

pub fn opt_float_args<T: This>(cx: &mut CallContext<'_, T>, rng: Range<usize>) -> Vec<f32>{
  let end = cmp::min(rng.end, cx.len() as usize);
  let rng = rng.start..end;
  rng.map(|i| cx.argument::<JsNumber>(i as i32))
     .filter( |r| r.is_ok() )
     .map( |num| num.as_ref().unwrap().value() as f32 )
     .collect()
}

pub fn float_args<T: This>(cx: &mut CallContext<'_, T>, rng: Range<usize>) -> Result<Vec<f32>, Throw>{
  let need = rng.end - rng.start;
  let list = opt_float_args(cx, rng);
  let got = list.len();
  match got == need{
    true => Ok(list),
    false => cx.throw_error(format!("expected {} numbers (got {})", need, got))
  }
}

//
// Colors
//


pub fn color_in<'a, T: This>(cx: &mut CallContext<'a, T>, css:&str) -> Result<Color, Throw> {
  match css.parse::<Rgba>(){
    Ok(Rgba{red, green, blue, alpha}) => {
      Ok(Color4f::new(red, green, blue, alpha).to_color())
    },
    Err(_e) => cx.throw_error("Color parse error: Invalid format")
  }
}

pub fn color_arg<'a, T: This>(cx: &mut CallContext<'a, T>, idx: usize) -> Result<Color, Throw> {
  match opt_string_arg(cx, idx){
    Some(css) => color_in(cx, &css),
    None => cx.throw_type_error("Expected a css color string, CanvasGradient, or CanvasPattern)")
  }
}

pub fn color_to_css<'a, T: This+Class>(cx: &mut CallContext<'a, T>, color:&Color) -> JsResult<'a, JsValue> {
  let RGB {r, g, b} = color.to_rgb();
  let css = match color.a() {
    255 => format!("#{:02x}{:02x}{:02x}", r, g, b),
    _ => {
      let alpha = format!("{:.3}", color.a() as f32 / 255.0);
      format!("rgba({},{},{},{})", r, g, b, alpha.trim_end_matches('0'))
    }
  };
  Ok(cx.string(css).upcast())
}

//
// Matrices
//

pub fn matrix_in<T: This>(cx: &mut CallContext<'_, T>, vals:&[Handle<JsValue>]) -> Result<Matrix, Throw>{
  // for converting single js-array args
  let terms = floats_in(vals);
  match to_matrix(&terms){
    Some(matrix) => Ok(matrix),
    None => cx.throw_error(format!("expected 6 or 9 matrix values (got {})", terms.len()))
  }
}

pub fn to_matrix(t:&[f32]) -> Option<Matrix>{
  match t.len(){
    6 => Some(Matrix::new_all(t[0], t[1], t[2], t[3], t[4], t[5], 0.0 as f32, 0.0 as f32, 1.0 as f32)),
    9 => Some(Matrix::new_all(t[0], t[1], t[2], t[3], t[4], t[5], t[6], t[7], t[8])),
    _ => None
  }
}

pub fn matrix_args<T: This>(cx: &mut CallContext<'_, T>, rng: Range<usize>) -> Result<Matrix, Throw>{
  // for converting inline args (e.g., in Path.transform())
  let terms = opt_float_args(cx, rng);
  match to_matrix(&terms){
    Some(matrix) => Ok(matrix),
    None => cx.throw_error(format!("expected 6 or 9 matrix values (got {})", terms.len()))
  }
}

pub fn matrix_arg<'a, T: This+Class>(cx: &mut CallContext<'a, T>, idx:usize) -> Result<Matrix, Throw> {
  let arg = cx.argument::<JsArray>(idx as i32)?.to_vec(cx)?;
  let terms = floats_in(&arg);
  match to_matrix(&terms){
    Some(matrix) => Ok(matrix),
    None => cx.throw_error(format!("expected 6 or 9 matrix values (got {})", terms.len()))
  }
}

pub fn matrix_to_array<'a, T: This+Class>(cx: &mut CallContext<'a, T>, matrix:&Matrix) -> JsResult<'a, JsValue> {
  let array = JsArray::new(cx, 9 as u32);
  for i in 0..9 {
    let num = cx.number(matrix[i as usize] as f64);
    array.set(cx, i as u32, num)?;
  }
  Ok(array.upcast())
}

//
// Points
//

pub fn points_in(vals:&[Handle<JsValue>]) -> Vec<Point>{
  floats_in(&vals).as_slice()
      .chunks(2)
      .map(|pair| Point::new(pair[0], pair[1]))
      .collect()
}

//
// Path2D
//

pub fn path2d_arg_opt<'a, T: This+Class>(cx: &mut CallContext<'a, T>, idx:usize) -> Option<Path> {
  if let Some(arg) = cx.argument_opt(idx as i32){
    if let Ok(arg) = arg.downcast::<JsPath2D>(){
      return Some(cx.borrow(&arg, |arg| arg.path.clone() ))
    }
  }
  None
}


//
// Fonts
//

pub struct FontSpec{
  pub families: Vec<String>,
  pub size: f32,
  pub style: FontStyle,
  pub features: Vec<(String, i32)>,
  pub variant: String,
  pub canonical: String
}

pub fn font_arg<'a, T: This>(cx: &mut CallContext<'a, T>, idx: usize) -> Result<Option<FontSpec>, Throw> {
  let arg = cx.argument::<JsValue>(0)?;
  if arg.is_a::<JsUndefined>(){ return Ok(None) }

  let font_desc = cx.argument::<JsObject>(idx as i32)?;
  let families = strings_at_key(cx, &font_desc, "family")?;
  let canonical = string_for_key(cx, &font_desc, "canonical")?;
  let variant = string_for_key(cx, &font_desc, "variant")?;
  let size = float_for_key(cx, &font_desc, "px")?;

  let weight = match float_for_key(cx, &font_desc, "wt")? as i32 {
    // https://docs.microsoft.com/en-us/typography/opentype/spec/os2#usweightclass
    wt if wt <= 100 => Weight::THIN,
    wt if wt <= 200 => Weight::EXTRA_LIGHT,
    wt if wt <= 300 => Weight::LIGHT,
    wt if wt <= 400 => Weight::NORMAL,
    wt if wt <= 500 => Weight::MEDIUM,
    wt if wt <= 600 => Weight::SEMI_BOLD,
    wt if wt <= 700 => Weight::BOLD,
    wt if wt <= 800 => Weight::EXTRA_BOLD,
    wt if wt < 950 => Weight::BLACK,
    _ => Weight::EXTRA_BLACK,
  };

  let slant = match string_for_key(cx, &font_desc, "style")?.as_str() {
    "italic" => Slant::Italic,
    "oblique" => Slant::Oblique,
    _ => Slant::Upright
  };

  let width = match string_for_key(cx, &font_desc, "stretch")?.as_str() {
    "ultra-condensed" => Width::ULTRA_CONDENSED,
    "extra-condensed" => Width::EXTRA_CONDENSED,
    "condensed" => Width::CONDENSED,
    "semi-condensed" => Width::SEMI_CONDENSED,
    "semi-expanded" => Width::SEMI_EXPANDED,
    "expanded" => Width::EXPANDED,
    "extra-expanded" => Width::EXTRA_EXPANDED,
    "ultra-expanded" => Width::ULTRA_EXPANDED,
    _ => Width::NORMAL,
  };

  let feat_obj = font_desc.get(cx, "features")?.downcast::<JsObject>().or_throw(cx)?;
  let features = font_features(cx, &feat_obj)?;

  let style = FontStyle::new(weight, width, slant);
  Ok(Some(FontSpec{ families, size, style, features, variant, canonical}))
}

pub fn font_features<T: This>(cx: &mut CallContext<'_, T>, obj: &Handle<JsObject>) -> Result<Vec<(String, i32)>, Throw>{
  let keys = obj.get_own_property_names(cx)?.to_vec(cx)?;
  let mut features:Vec<(String, i32)> = vec![];
  for key in strings_in(&keys).iter() {
    match key.as_str() {
      "on" | "off" => strings_at_key(cx, obj, key)?.iter().for_each(|feat|{
        features.push( (feat.to_string(), if key == "on"{ 1 } else { 0 }) );
      }),
      _ => features.push( (key.to_string(), float_for_key(cx, obj, key)? as i32))
    }
  }
  Ok(features)
}

//
// Filters
//

pub enum FilterSpec{
  Plain{name:String, value:f32},
  Shadow{offset:Point, blur:f32, color:Color},
}

pub fn filter_arg<'a, T: This>(cx: &mut CallContext<'a, T>, idx: usize) -> Result<(String, Vec<FilterSpec>), Throw> {
  let arg = cx.argument::<JsObject>(idx as i32)?;
  let canonical = string_for_key(cx, &arg, "canonical")?;

  let obj = arg.get(cx, "filters")?.downcast_or_throw::<JsObject, _>(cx)?;
  let keys = obj.get_own_property_names(cx)?.to_vec(cx)?;
  let mut filters = vec![];
  for (name, key) in strings_in(&keys).iter().zip(keys) {
    match name.as_str() {
      "drop-shadow" => {
        let values = obj.get(cx, key)?.downcast_or_throw::<JsArray, _>(cx)?;
        let dims = floats_in(&values.to_vec(cx)?);
        let color = values.get(cx, 3)?.downcast_or_throw::<JsString, _>(cx)?.value();
        filters.push(FilterSpec::Shadow{
          offset: Point::new(dims[0], dims[1]), blur: dims[2], color:color_in(cx, &color)?
        });
      },
      _ => {
        let value = obj.get(cx, key)?.downcast_or_throw::<JsNumber, _>(cx)?.value();
        filters.push(FilterSpec::Plain{
          name:name.to_string(), value:value as f32
        })
      }
    }
  }
  Ok( (canonical, filters) )
}

//
// Skia Enums
//

use skia_safe::{TileMode};
pub fn to_tile_mode(mode_name:&str) -> Option<TileMode>{
  let mode = match mode_name.to_lowercase().as_str(){
    "clamp" => TileMode::Clamp,
    "repeat" => TileMode::Repeat,
    "mirror" => TileMode::Mirror,
    "decal" => TileMode::Decal,
    _ => return None
  };
  Some(mode)
}

use skia_safe::{FilterQuality};
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

use skia_safe::textlayout::{TextAlign, TextDirection, ParagraphStyle};
pub fn to_text_align(mode_name:&str) -> Option<TextAlign>{
  let mode = match mode_name.to_lowercase().as_str(){
    "left" => TextAlign::Left,
    "right" => TextAlign::Right,
    "center" => TextAlign::Center,
    // "justify" => TextAlign::Justify,
    "start" => TextAlign::Start,
    "end" => TextAlign::End,
    _ => return None
  };
  Some(mode)
}

pub fn from_text_align(mode:TextAlign) -> String{
  match mode{
    TextAlign::Left => "left",
    TextAlign::Right => "right",
    TextAlign::Center => "center",
    TextAlign::Justify => "justify",
    TextAlign::Start => "start",
    TextAlign::End => "end",
  }.to_string()
}

pub fn get_alignment_factor(graf_style:&ParagraphStyle) -> f32 {
  match graf_style.text_direction() {
    TextDirection::LTR => match graf_style.text_align() {
      TextAlign::Left | TextAlign::Start => 0.0,
      TextAlign::Right | TextAlign::End => -1.0,
      TextAlign::Center => -0.5,
      TextAlign::Justify => 0.0 // unsupported
    },
    TextDirection::RTL => match graf_style.text_align() {
      TextAlign::Left | TextAlign::End => 0.0,
      TextAlign::Right | TextAlign::Start => -1.0,
      TextAlign::Center => -0.5,
      TextAlign::Justify => 0.0 // unsupported
    }
  }
}



#[derive(Copy, Clone)]
pub enum Baseline{ Top, Hanging, Middle, Alphabetic, Ideographic, Bottom }

pub fn to_text_baseline(mode_name:&str) -> Option<Baseline>{
  let mode = match mode_name.to_lowercase().as_str(){
    "top" => Baseline::Top,
    "hanging" => Baseline::Hanging,
    "middle" => Baseline::Middle,
    "alphabetic" => Baseline::Alphabetic,
    "ideographic" => Baseline::Ideographic,
    "bottom" => Baseline::Bottom,
    _ => return None
  };
  Some(mode)
}

pub fn from_text_baseline(mode:Baseline) -> String{
  match mode{
    Baseline::Top => "top",
    Baseline::Hanging => "hanging",
    Baseline::Middle => "middle",
    Baseline::Alphabetic => "alphabetic",
    Baseline::Ideographic => "ideographic",
    Baseline::Bottom => "bottom",
  }.to_string()
}

pub fn get_baseline_offset(metrics: &FontMetrics, mode:Baseline) -> f64 {
  match mode{
    Baseline::Top => -metrics.ascent as f64,
    Baseline::Hanging => metrics.cap_height as f64,
    Baseline::Middle => metrics.cap_height as f64 / 2.0,
    Baseline::Alphabetic => 0.0,
    Baseline::Ideographic => -metrics.descent as f64,
    Baseline::Bottom => -metrics.descent as f64,
  }
}



use skia_safe::path::FillType;
pub fn fill_rule_arg_or<T: This>(cx: &mut CallContext<'_, T>, idx: usize, default: &str) -> Result<FillType, Throw>{
  let rule = match string_arg_or(cx, idx, default).as_str(){
    "nonzero" => FillType::Winding,
    "evenodd" => FillType::EvenOdd,
    _ => {
      let err_msg = format!("Argument {} ('fillRule') must be one of: \"nonzero\", \"evenodd\"", idx);
      return cx.throw_type_error(err_msg)
    }
  };
  Ok(rule)
}

pub fn blend_mode_arg<T: This>(cx: &mut CallContext<'_, T>, idx: usize, attr: &str) -> Result<BlendMode, Throw>{
  let mode_name = string_arg(cx, idx, attr)?;
  match to_blend_mode(&mode_name){
    Some(blend_mode) => Ok(blend_mode),
    None => cx.throw_error("blendMode must be SrcOver, DstOver, Src, Dst, Clear, SrcIn, DstIn, \
                            SrcOut, DstOut, SrcATop, DstATop, Xor, Plus, Multiply, Screen, Overlay, \
                            Darken, Lighten, ColorDodge, ColorBurn, HardLight, SoftLight, Difference, \
                            Exclusion, Hue, Saturation, Color, Luminosity, or Modulate")
  }
}
