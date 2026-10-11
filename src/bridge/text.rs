use core::ops::Range;
use skia_safe::Typeface;
use skia_safe::font_style::{FontStyle, Weight, Width, Slant};
use skia_safe::textlayout::{TextAlign, TextDecorationStyle};

use super::*;
use crate::typography::{Baseline, DecorationKind, DecorationLine, DecorationStyle, TextRendering};
use crate::opentype::FontCapabilities;

//
// Font argument packing & unpacking
//
#[derive(Debug, Clone)]
pub struct FontSpec{
  pub families: Vec<String>,
  pub size: f32,
  pub line_height: Option<f32>,
  pub weight: Weight,
  pub width: Width,
  pub slant: Slant,
  pub oblique: Option<f32>,
  pub features: Vec<(String, i32)>,
  pub variant: String,
  pub canonical: String
}

impl FontSpec{
  pub fn style(&self) -> FontStyle{
    FontStyle::new(self.weight, self.width, self.slant)
  }
}

impl Default for FontSpec{
  fn default() -> Self{
    FontSpec{
      families: vec!["sans-serif".to_string()],
      size: 10.0,
      line_height: None,
      weight: Weight::NORMAL,
      width: Width::NORMAL,
      slant: Slant::Upright,
      oblique: None,
      features: vec![],
      variant: "normal".to_string(),
      canonical: "10px sans-serif".to_string(),
    }
  }
}

pub fn font_arg(cx: &mut FunctionContext, idx: usize) -> NeonResult<Option<FontSpec>> {
  let arg = cx.argument::<JsValue>(idx)?;
  if arg.is_a::<JsNull, _>(cx){ return Ok(None) }

  let font_desc = cx.argument::<JsObject>(idx)?;
  let families = strings_at_key(cx, &font_desc, "family")?;
  let canonical = string_for_key(cx, &font_desc, "canonical")?;
  let variant = string_for_key(cx, &font_desc, "variant")?;
  let size = float_for_key(cx, &font_desc, "size")?;
  let weight = Weight::from(float_for_key(cx, &font_desc, "weight")? as i32);
  let slant = to_slant(string_for_key(cx, &font_desc, "style")?.as_str());
  let oblique = opt_float_for_key(cx, &font_desc, "obliqueAngle");
  let width = to_width(string_for_key(cx, &font_desc, "stretch")?.as_str());
  let line_height = opt_float_for_key(cx, &font_desc, "lineHeight")
    .map(|pt_size| pt_size / size);

  let feat_obj:Handle<JsObject> = font_desc.get(cx, "features")?;
  let features = font_features(cx, &feat_obj)?;

  Ok(match families[0].is_empty(){
    true => None, // silently fail if a family name was omitted (e.g., "bold 50px")
    false => Some(FontSpec{ families, size, line_height, weight, slant, oblique, width, features, variant, canonical})
  })
}

pub fn font_features(cx: &mut FunctionContext, obj: &Handle<JsObject>) -> NeonResult<Vec<(String, i32)>>{
  let keys = obj.get_own_property_names(cx)?.to_vec(cx)?;
  let mut features:Vec<(String, i32)> = vec![];
  for key in strings_in(cx, &keys)?.iter() {
    match key.as_str() {
      "on" | "off" => strings_at_key(cx, obj, key)?.iter().for_each(|feat|{
        features.push( (feat.to_string(), if key == "on"{ 1 } else { 0 }) );
      }),
      _ => features.push( (key.to_string(), float_for_key(cx, obj, key)? as i32))
    }
  }
  Ok(features)
}

pub fn to_slant(slant_name:&str) -> Slant{
  match slant_name.to_lowercase().as_str(){
    "italic" => Slant::Italic,
    "oblique" => Slant::Oblique,
    _ => Slant::Upright
  }
}

pub fn from_slant(slant:Slant) -> String{
  match slant {
    Slant::Upright => "normal",
    Slant::Italic => "italic",
    Slant::Oblique => "oblique",
  }.to_string()
}

pub fn to_width(width_name:&str) -> Width{
  match width_name.to_lowercase().as_str(){
    "ultra-condensed" => Width::ULTRA_CONDENSED,
    "extra-condensed" => Width::EXTRA_CONDENSED,
    "condensed" => Width::CONDENSED,
    "semi-condensed" => Width::SEMI_CONDENSED,
    "semi-expanded" => Width::SEMI_EXPANDED,
    "expanded" => Width::EXPANDED,
    "extra-expanded" => Width::EXTRA_EXPANDED,
    "ultra-expanded" => Width::ULTRA_EXPANDED,
    _ => Width::NORMAL,
  }
}

pub fn from_width(width:Width) -> String{
  match width {
    w if w == Width::ULTRA_CONDENSED => "ultra-condensed",
    w if w == Width::EXTRA_CONDENSED => "extra-condensed",
    w if w == Width::CONDENSED => "condensed",
    w if w == Width::SEMI_CONDENSED => "semi-condensed",
    w if w == Width::SEMI_EXPANDED => "semi-expanded",
    w if w == Width::EXPANDED => "expanded",
    w if w == Width::EXTRA_EXPANDED => "extra-expanded",
    w if w == Width::ULTRA_EXPANDED => "ultra-expanded",
    _ => "normal"
  }.to_string()
}

pub fn to_text_align(mode_name:&str) -> Option<TextAlign>{
  let mode = match mode_name.to_lowercase().as_str(){
    "left" => TextAlign::Left,
    "right" => TextAlign::Right,
    "center" => TextAlign::Center,
    "justify" => TextAlign::Justify,
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

pub fn to_text_rendering(mode_name:&str) -> Option<TextRendering>{
  let mode = match mode_name.to_lowercase().as_str(){
    "auto" => TextRendering::Auto,
    "optimizespeed" => TextRendering::OptimizeSpeed,
    "optimizelegibility" => TextRendering::OptimizeLegibility,
    "geometricprecision" => TextRendering::GeometricPrecision,
    _ => return None
  };
  Some(mode)
}

pub fn from_text_rendering(mode:TextRendering) -> String{
  match mode{
    TextRendering::Auto => "auto",
    TextRendering::OptimizeSpeed => "optimizeSpeed",
    TextRendering::OptimizeLegibility => "optimizeLegibility",
    TextRendering::GeometricPrecision => "geometricPrecision",
  }.to_string()
}

pub fn decoration_arg(cx: &mut FunctionContext, idx: usize) -> NeonResult<Option<DecorationStyle>> {
  if let Some(deco) = opt_object_arg(cx, idx){
    let css = string_for_key(cx, &deco, "str")?;

    // inherit the fill color unless textDecoration specifies a css color to use
    let color = match string_for_key(cx, &deco, "color")?.as_str(){
      clr if clr.eq_ignore_ascii_case("currentColor") => None,
      color_str => match CssColor::parse(color_str){
        Some(color) => Some(color),
        None => return cx.throw_type_error(format!("⚠️Invalid text decoration: {:?}", css)),
      }
    };

    let line = string_for_key(cx, &deco, "line")?;
    let kind = match line.as_str(){
      "underline" => DecorationKind::Underline,
      "overline" => DecorationKind::Overline,
      "line-through" => DecorationKind::LineThrough,
      "none" | _ => return Ok(Some(DecorationStyle::default()))
    };

    let line_style = string_for_key(cx, &deco, "style")?;
    let style = match line_style.as_str(){
      "wavy" => TextDecorationStyle::Wavy,
      "dotted" => TextDecorationStyle::Dotted,
      "dashed" => TextDecorationStyle::Dashed,
      "double" => TextDecorationStyle::Double,
      "solid" | _ => TextDecorationStyle::Solid,
    };

    let inherit = string_for_key(cx, &deco, "inherit")?;
    let size = match inherit.as_str(){
      "from-font" => None,
      _ => match opt_object_for_key(cx, &deco, "thickness"){
        Some(thickness) => Spacing::from_obj(cx, &thickness)?,
        _ => None
      }
    };

    Ok(Some(DecorationStyle{ css, line:Some(DecorationLine{ kind, style }), size, color }))
  }else{
    Ok(None)
  }
}

// font-stretch keywords and their corresponding percentages
const STRETCH_BUCKETS: [(Width, f32); 9] = [
  (Width::ULTRA_CONDENSED, 50.0),  (Width::EXTRA_CONDENSED, 62.5),
  (Width::CONDENSED,       75.0),  (Width::SEMI_CONDENSED,  87.5),
  (Width::NORMAL,         100.0),  (Width::SEMI_EXPANDED,  112.5),
  (Width::EXPANDED,       125.0),  (Width::EXTRA_EXPANDED, 150.0),
  (Width::ULTRA_EXPANDED, 200.0),
];

// convert from a keyword-based Width to a `wdth` percentage
pub fn width_percent(width:Width) -> f32{
  STRETCH_BUCKETS.iter().find(|(w, _)| *w == width).map(|(_, p)| *p).unwrap_or(100.0)
}

// find the nearest Width to a given `wdth` percentage
pub fn nearest_width(pct:f32) -> Width{
  STRETCH_BUCKETS.iter()
    .min_by(|(_, a), (_, b)| (a - pct).abs().total_cmp(&(b - pct).abs()))
    .map(|(w, _)| *w)
    .unwrap_or(Width::NORMAL)
}

// convert percentage to a css keyword string (if one matches) or a percent string
pub fn stretch_label(pct:f32) -> String{
  match STRETCH_BUCKETS.iter().find(|(_, p)| *p == pct){
    Some((w, _)) => from_width(*w),
    None => format!("{}%", pct),
  }
}

// parse a css keyword or percentage string into a `wdth` percentage
pub fn font_stretch_arg(cx: &mut FunctionContext, idx: usize) -> NeonResult<Option<f32>> {
  if let Some(percent) = opt_float_arg(cx, idx){
    Ok(Some(percent))
  }else if let Some(keyword) = opt_string_arg(cx, idx){
    let lower = keyword.trim().to_lowercase();
    let width = to_width(&lower); // map non-matches to NORMAL
    match from_width(width) == lower{
      true => Ok(Some(width_percent(width))),
      false => cx.throw_type_error(format!("⚠️Invalid font stretch: {:?}", keyword)),
    }
  }else{
    Ok(None)
  }
}

//
// Font capabilities (variation axes and OpenType features)
//

pub fn typeface_details<'a>(cx: &mut FunctionContext<'a>, filename:&str, font: &Typeface, alias:Option<String>) -> JsResult<'a, JsObject> {
  let style = font.font_style();
  let caps = FontCapabilities::new(font);
  let variations = caps.variations_object(cx)?;
  let features = caps.features_object(cx)?;
  let instances = caps.instances_object(cx, false)?;

  let dict = JsObject::new(cx);
  dict.prop(cx, "family").set(alias.unwrap_or_else(|| font.family_name()))?;
  dict.prop(cx, "weight").set(*style.weight() as f64)?;
  dict.prop(cx, "style").set(from_slant(style.slant()))?;
  dict.prop(cx, "width").set(from_width(style.width()))?;
  dict.prop(cx, "variable").set(!caps.axes.is_empty())?;
  dict.prop(cx, "variations").set(variations)?;
  dict.prop(cx, "instances").set(instances)?;
  dict.prop(cx, "features").set(features)?;
  dict.prop(cx, "file").set(filename)?;
  Ok(dict)
}

//
// fontVariationSettings & fontFeatureSettings unpacking
//

// unpack the js `fontVariationSettings` mapping
pub fn variation_settings(cx: &mut FunctionContext, obj: &Handle<JsObject>) -> NeonResult<Vec<(String, f32)>>{
  let keys = obj.get_own_property_names(cx)?.to_vec(cx)?;
  let mut settings:Vec<(String, f32)> = vec![];
  for key in strings_in(cx, &keys)?.iter() {
    settings.push( (key.to_string(), float_for_key(cx, obj, key)?) );
  }
  Ok(settings)
}

// unpack the `{tag: value, …}` map sent by the js `fontFeatureSettings` parser
pub fn feature_settings(cx: &mut FunctionContext, obj: &Handle<JsObject>) -> NeonResult<Vec<(String, i32)>>{
  let keys = obj.get_own_property_names(cx)?.to_vec(cx)?;
  let mut settings:Vec<(String, i32)> = vec![];
  for key in strings_in(cx, &keys)?.iter() {
    settings.push( (key.to_string(), float_for_key(cx, obj, key)? as i32) );
  }
  Ok(settings)
}

//
// Em-relative lengths (for text spacing & decoration thickness)
//

#[derive(Clone, Debug)]
pub struct Spacing{
  raw_size: f32,
  unit: String,
  px_size: f32,
}

impl Default for Spacing{
  fn default() -> Self {
    Self{raw_size:0.0, unit:"px".to_string(), px_size:0.0}
  }
}

impl Spacing{
  pub fn from_obj(cx: &mut FunctionContext, spacing:&Handle<JsObject>) -> NeonResult<Option<Self>>{
    let raw_size = float_for_key(cx, spacing, "size")?;
    let unit = string_for_key(cx, spacing, "unit")?;
    let px_size = float_for_key(cx, spacing, "px")?;
    Ok(Self::parse(raw_size, unit, px_size))
  }

  pub fn parse(raw_size:f32, unit:String, px_size:f32) -> Option<Self>{
    let main_size = match unit.as_str(){
      "em" | "rem" => raw_size,
      _ => px_size
    };

    match main_size.is_nan(){
      false => Some(Self{raw_size, unit, px_size}),
      true => None
    }
  }

  pub fn in_px(&self, em_size:f32) -> f32{
    match self.unit.as_str(){
      "em" => self.raw_size * em_size,
      "rem" => self.raw_size * 16.0,
      _ => self.px_size
    }
  }

  pub fn to_string(&self) -> String{
    format!("{}{}", self.raw_size, self.unit)
  }
}

pub fn opt_spacing_arg<'a>(cx: &mut FunctionContext<'a>, idx:usize) -> NeonResult<Option<Spacing>>{
  match cx.argument::<JsValue>(idx)?.is_a::<JsNull, _>(cx){
    true => Ok(None),
    false => {
      let spacing = cx.argument::<JsObject>(idx)?;
      Spacing::from_obj(cx, &spacing)
    }
  }
}

//
// Convert utf-8 byte indices -> utf-16 codepoint indices
//

pub struct Utf16Offsets(Vec<u32>); // u32 halves the table and still indexes any string a JS engine can hold

impl Utf16Offsets{
  pub fn new(text:&str) -> Self{
    let mut table = Vec::with_capacity(text.len() + 1);
    let mut offset = 0u32;
    for c in text.chars(){
      table.push(offset);
      offset += c.len_utf16() as u32;
      // bytes inside a multi-byte char round up to the following index
      table.extend(std::iter::repeat(offset).take(c.len_utf8() - 1));
    }
    table.push(offset);
    Utf16Offsets(table)
  }

  pub fn range(&self, byte_range:&Range<usize>) -> Range<usize>{
    let at = |byte:usize| self.0[byte.min(self.0.len() - 1)] as usize;
    at(byte_range.start)..at(byte_range.end)
  }
}
