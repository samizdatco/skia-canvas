#![allow(non_snake_case)]
use core::ops::Range;
use neon::prelude::*;
use skia_safe::{Typeface, FourByteTag, FontArguments};
use skia_safe::font_style::{FontStyle, Weight, Width, Slant};
use skia_safe::font_arguments::{VariationPosition, variation_position::Coordinate};
use skia_safe::textlayout::{TextAlign, TextDecorationStyle, TextStyle};

use super::*;
use crate::typography::{Baseline, DecorationKind, DecorationLine, DecorationStyle};

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

  Ok(match families[0] == ""{
    true => None, // silently fail if a family name was omitted (e.g., "bold 50px")
    false => Some(FontSpec{ families, size, line_height, weight, slant, oblique, width, features, variant, canonical})
  })
}

pub fn font_features(cx: &mut FunctionContext, obj: &Handle<JsObject>) -> NeonResult<Vec<(String, i32)>>{
  let keys = obj.get_own_property_names(cx)?.to_vec(cx)?;
  let mut features:Vec<(String, i32)> = vec![];
  for key in strings_in(cx, &keys).iter() {
    match key.as_str() {
      "on" | "off" => strings_at_key(cx, obj, key)?.iter().for_each(|feat|{
        features.push( (feat.to_string(), if key == "on"{ 1 } else { 0 }) );
      }),
      _ => features.push( (key.to_string(), float_for_key(cx, obj, key)? as i32))
    }
  }
  Ok(features)
}

pub fn typeface_details<'a>(cx: &mut FunctionContext<'a>, filename:&str, font: &Typeface, alias:Option<String>) -> JsResult<'a, JsObject> {
  let style = font.font_style();

  let filename = cx.string(filename);
  let family = cx.string(match alias{
    Some(name) => name,
    None => font.family_name()
  });
  let weight = cx.number(*style.weight() as f64);
  let slant = cx.string(from_slant(style.slant()));
  let width = cx.string(from_width(style.width()));

  let dict = JsObject::new(cx);
  let attr = cx.string("family"); dict.set(cx, attr, family)?;
  let attr = cx.string("weight"); dict.set(cx, attr, weight)?;
  let attr = cx.string("style");  dict.set(cx, attr, slant)?;
  let attr = cx.string("width");  dict.set(cx, attr, width)?;
  let attr = cx.string("file");   dict.set(cx, attr, filename)?;
  Ok(dict)
}

pub fn typeface_wght_range(font:&Typeface) -> Vec<i32>{
  let mut wghts = vec![];
  if let Some(params) = font.variation_design_parameters(){
    for param in params {
      let chars = vec![param.tag.a(), param.tag.b(), param.tag.c(), param.tag.d()];
      let tag = String::from_utf8(chars).unwrap();
      let (min, max) = (param.min as i32, param.max as i32);
      if tag == "wght"{
        let mut val = min;
        while val <= max {
          wghts.push(val);
          val = val + 100 - (val % 100);
        }
        if !wghts.contains(&max){
          wghts.push(max);
        }
      }
    }
  }
  wghts
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

pub fn decoration_arg(cx: &mut FunctionContext, idx: usize) -> NeonResult<Option<DecorationStyle>> {
  if let Some(deco) = opt_object_arg(cx, idx){
    let css = string_for_key(cx, &deco, "str")?;

    // inherit the fill color unless textDecoration specifies a css color to use
    let color = match string_for_key(cx, &deco, "color")?.as_str(){
      "currentColor" => None,
      color_str => match CssColor::parse(&color_str){
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

#[derive(Clone, Debug, Default)]
pub struct FontAxes{
  variations: Vec<(String, f32)>, // decoded fontVariationSettings
}

impl FontAxes{
  pub fn set_variations(&mut self, mut variations:Vec<(String, f32)>){
    variations.sort_by(|(a, _), (b, _)| a.cmp(b));
    self.variations = variations;
  }

  // reconstruct the canonical css serialization
  pub fn css_string(&self) -> String{
    if self.variations.is_empty(){ return "normal".to_string() }
    self.variations.iter()
      .map(|(tag, value)| format!("\"{}\" {}", tag, value))
      .collect::<Vec<_>>()
      .join(", ")
  }

  pub fn clear_variations(&mut self){
    self.variations.clear();
  }

  // merge the variable font instancing fields into the provided char_style
  pub fn apply(&self, char_style:&mut TextStyle, stretch:f32, oblique:Option<f32>){
    let font_style = char_style.font_style();
    let axes = char_style.typeface().and_then(|tf| tf.variation_design_parameters());
    let has_axis = |tag| axes.as_ref().is_some_and(|params| params.iter().any(|p| p.tag == tag));
    let slant_only_font = has_axis(Coordinate::slnt) && !has_axis(Coordinate::ital);

    let mut coords = vec![
      Coordinate{ axis:Coordinate::wght, value:*font_style.weight() as f32 },
      Coordinate{ axis:Coordinate::wdth, value:stretch },
    ];
    match (font_style.slant(), slant_only_font){
      // for slnt-only fonts, map an italic style to the default oblique angle
      (Slant::Italic, true)  => coords.push(Coordinate{ axis:Coordinate::slnt, value:-14.0 }),
      (Slant::Italic, false) => coords.push(Coordinate{ axis:Coordinate::ital, value:1.0 }),
      // if oblique is explicitly selected, disable ital to ensure roman letterforms
      (Slant::Oblique, _) => coords.extend([
        Coordinate{ axis:Coordinate::ital, value:0.0 },
        Coordinate{ axis:Coordinate::slnt, value:-oblique.unwrap_or(14.0) },
      ]),
      _ => coords.push(Coordinate{ axis:Coordinate::ital, value:0.0 }),
    }
    for (tag, value) in &self.variations{
      let b = tag.as_bytes();
      let axis = FourByteTag::from_chars(b[0] as char, b[1] as char, b[2] as char, b[3] as char);
      coords.push(Coordinate{ axis, value:*value });
    }

    let args = FontArguments::new()
      .set_variation_design_position(VariationPosition{ coordinates: &coords });
    char_style.set_font_arguments(&args);
  }

  // state the measureText cache depends on
  pub fn cache_key(&self) -> Vec<(String, u32)>{
    self.variations.iter().map(|(tag, value)| (tag.clone(), value.to_bits())).collect()
  }
}

// unpack the js `fontVariationSettings` mapping
pub fn variation_settings(cx: &mut FunctionContext, obj: &Handle<JsObject>) -> NeonResult<Vec<(String, f32)>>{
  let keys = obj.get_own_property_names(cx)?.to_vec(cx)?;
  let mut settings:Vec<(String, f32)> = vec![];
  for key in strings_in(cx, &keys).iter() {
    settings.push( (key.to_string(), float_for_key(cx, obj, key)?) );
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
    let raw_size = float_for_key(cx, &spacing, "size")?;
    let unit = string_for_key(cx, &spacing, "unit")?;
    let px_size = float_for_key(cx, &spacing, "px")?;
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

pub fn utf16_range(text:&str, byte_range:&Range<usize>) -> Range<usize>{
  let chars:Vec<(usize, usize)> = text.char_indices()
    .map(|(idx, c)| (idx, c.len_utf16()))
    .collect::<Vec<(usize, usize)>>();

  // find the char indices corresponding to the byte range endpoints
  let start = chars.iter().position(|(i, _)| *i >= byte_range.start).unwrap_or(0);
  let end = chars.iter().rposition(|(i, _)| *i < byte_range.end).map(|i| i + 1).unwrap_or(start);

  // sum up the number of utf-16 code units needed for all chars in the range
  let sum = |a,b|{a+b};
  let len = |&(_, len)|{len};
  let head = chars.iter().take(start).map(len).reduce(sum).unwrap_or(0);
  let tail = chars.iter().skip(start).take(end-start).map(len).reduce(sum).unwrap_or(head);
  head..head+tail
}
