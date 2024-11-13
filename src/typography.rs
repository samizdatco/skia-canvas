#![allow(unused_variables)]
#![allow(unused_mut)]
#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(non_snake_case)]
use std::ops::Range;
use neon::prelude::*;

use skia_safe::{FontMetrics, Typeface, Paint, Point, Rect, Path as SkPath, Color};
use skia_safe::font_style::{FontStyle, Weight, Width, Slant};
use skia_safe::textlayout::{
    FontCollection, TextStyle, TextAlign, TextDirection,
    Decoration, TextDecoration, TextDecorationMode, TextDecorationStyle,
    ParagraphStyle, Paragraph, ParagraphBuilder,
};
use crate::FONT_LIBRARY;
use crate::utils::*;
use crate::context::State;

//
// Text layout and metrics
//

const GALLEY:f32 = 100_000.0;

pub struct Typesetter{
  text: String,
  width: f32,
  baseline: Baseline,
  typefaces: FontCollection,
  char_style: TextStyle,
  graf_style: ParagraphStyle,
  text_decoration: DecorationStyle,
}

impl Typesetter{
  pub fn new(state:&State, text: &str, width:Option<f32>) -> Self {
    let mut library = FONT_LIBRARY.lock().unwrap();
    let (char_style, mut graf_style, text_decoration, baseline, wrap) = state.typography();
    let typefaces = library.collect_fonts(&char_style);
    let width = width.unwrap_or(GALLEY);
    let text = match wrap{
      true => text.to_string(),
      false => {
        graf_style.set_max_lines(1);
        text.replace("\n", " ")
      }
    };

    if wrap {
      // make sure line-breaks use the current leading
      let mut strut_style = graf_style.strut_style().clone();
      let (leading, size) = if char_style.height() < 1.0 {
        ( strut_style.leading(), char_style.font_size() * char_style.height() )
      }else{
        ( char_style.height() - 1.0, char_style.font_size() )
      };
      strut_style
        .set_strut_enabled(true)
        .set_force_strut_height(true)
        .set_font_size(size)
        .set_leading(leading);
      graf_style.set_strut_style(strut_style);
    }

    Typesetter{text, width, baseline, typefaces, char_style, graf_style, text_decoration}
  }

  pub fn layout(&self, paint:&Paint) -> (Paragraph, Point) {
    let mut char_style = self.char_style.clone();
    char_style.set_foreground_paint(paint);
    char_style.set_decoration(
      &self.text_decoration.for_layout(&char_style, paint.color())
    );

    // prevent SkParagraph from faking of the font style if the match isn't the requested weight/slant
    let fams:Vec<String> = char_style.font_families().iter().map(|s| s.to_string()).collect();
    if let Some(matched) = self.typefaces.clone().find_typefaces(&fams, char_style.font_style()).first(){
      char_style.set_font_style(matched.font_style());
    }

    let mut paragraph_builder = ParagraphBuilder::new(&self.graf_style, &self.typefaces);
    paragraph_builder.push_style(&char_style);
    paragraph_builder.add_text(&self.text);

    let mut paragraph = paragraph_builder.build();
    paragraph.layout(self.width);

    let metrics = self.char_style.font_metrics();
    let shift = get_baseline_offset(&metrics, self.baseline);
    let offset = (
      self.width * get_alignment_factor(&self.graf_style),
      shift - paragraph.alphabetic_baseline(),
    );

    (paragraph, offset.into())
  }

  pub fn metrics(&self) -> Vec<Vec<f32>>{
    let (paragraph, _) = self.layout(&Paint::default());
    let font_metrics = self.char_style.font_metrics();
    let offset = get_baseline_offset(&font_metrics, self.baseline);
    let hang = get_baseline_offset(&font_metrics, Baseline::Hanging) - offset;
    let norm = get_baseline_offset(&font_metrics, Baseline::Alphabetic) - offset;
    let ideo = get_baseline_offset(&font_metrics, Baseline::Ideographic) - offset;
    let ascent = norm - font_metrics.ascent;
    let descent = font_metrics.descent - norm;
    let alignment = get_alignment_factor(&self.graf_style) * self.width;

    if paragraph.line_number() == 0 {
      return vec![vec![0.0, 0.0, 0.0, 0.0, 0.0, ascent, descent, ascent, descent, hang, norm, ideo]]
    }

    // find the bounds and text-range for each individual line
    let origin = paragraph.get_line_metrics()[0].baseline;
    let line_rects:Vec<(Rect, Range<usize>, f32)> = paragraph.get_line_metrics().iter().map(|line|{
      let baseline = line.baseline - origin;
      let rect = Rect::new(line.left as f32, (baseline - line.ascent) as f32,
                          (line.left + line.width) as f32, (baseline + line.descent) as f32);
      let range = string_idx_range(&self.text, line.start_index,
        if self.width==GALLEY{ line.end_index }else{ line.end_excluding_whitespaces }
      );
      (rect.with_offset((alignment, offset)), range, baseline as f32 + offset)
    }).collect();

    // take their union to find the bounds for the whole text run
    let (bounds, chars) = line_rects.iter().fold((Rect::new_empty(), 0), |(union, indices), (rect, range, _)|
      (Rect::join2(union, rect), range.end)
    );

    // return a list-of-lists whose first entry is the whole-run font metrics and subsequent entries are
    // line-rect/range values (with the js side responsible for restructuring the whole bundle)
    let mut results = vec![vec![
      bounds.width(), bounds.left, bounds.right, -bounds.top, bounds.bottom,
      ascent, descent, ascent, descent, hang, norm, ideo
    ]];
    line_rects.iter().for_each(|(rect, range, baseline)|{
      results.push(vec![rect.left, rect.top, rect.width(), rect.height(),
                        *baseline, range.start as f32, range.end as f32])
    });
    results
  }

  pub fn path(&mut self) -> SkPath {
    let (mut paragraph, mut offset) = self.layout(&Paint::default());
    offset.y -= self.char_style.font_metrics().ascent + paragraph.alphabetic_baseline();

    let mut path = SkPath::new();
    for idx in 0..paragraph.line_number(){
      let (skipped, line) = paragraph.get_path_at(idx);
      path.add_path(&line, offset, None);
    };
    path
  }
}

//
// Font argument packing & unpacking
//
#[derive(Debug, Clone)]
pub struct FontSpec{
  pub families: Vec<String>,
  pub size: f32,
  pub leading: f32,
  pub weight: Weight,
  pub width: Width,
  pub slant: Slant,
  pub features: Vec<(String, i32)>,
  pub variant: String,
  pub canonical: String
}

impl FontSpec{
  pub fn with_width(&self, width:Width) -> Self{
    Self{width, ..self.clone()}
  }

  pub fn style(&self) -> FontStyle{
    FontStyle::new(self.weight, self.width, self.slant)
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
  let leading = float_for_key(cx, &font_desc, "lineHeight")?;

  let weight = Weight::from(float_for_key(cx, &font_desc, "weight")? as i32);
  let slant = to_slant(string_for_key(cx, &font_desc, "style")?.as_str());
  let width = to_width(string_for_key(cx, &font_desc, "stretch")?.as_str());

  let feat_obj:Handle<JsObject> = font_desc.get(cx, "features")?;
  let features = font_features(cx, &feat_obj)?;

  Ok(Some(FontSpec{ families, size, leading, weight, slant, width, features, variant, canonical}))
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

#[derive(Copy, Clone, Debug)]
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

pub fn get_baseline_offset(metrics: &FontMetrics, mode:Baseline) -> f32 {
  match mode{
    Baseline::Top => -metrics.ascent,
    Baseline::Hanging => metrics.cap_height,
    Baseline::Middle => metrics.cap_height / 2.0,
    Baseline::Alphabetic => 0.0,
    Baseline::Ideographic => -metrics.descent,
    Baseline::Bottom => -metrics.descent,
  }
}

#[derive(Clone, Debug)]
pub struct DecorationStyle{
  pub css: String,
  pub decoration: Decoration,
  pub size: Option<Spacing>,
  pub color: Option<Color>,
}


impl Default for DecorationStyle{
  fn default() -> Self {
    Self{decoration:Decoration::default(), size:None, color:None, css:"none".to_string()}
  }
}

impl DecorationStyle{
  pub fn for_layout(&self, style:&TextStyle, text_color:Color) -> Decoration{
    // convert `size` into a multiple of the current font's default thickness
    let em_size = style.font_size();
    let thickness = style.font_metrics()
      .underline_thickness()
      .unwrap_or(1.0);
    let thickness_multiplier = self.size.clone()
      .map(|size| size.in_px(em_size) / thickness)
      .unwrap_or(1.0);
    let color = self.color.unwrap_or(text_color);
    Decoration{thickness_multiplier, color, ..self.decoration}
  }
}

pub fn decoration_arg(cx: &mut FunctionContext, idx: usize) -> NeonResult<Option<DecorationStyle>> {
  if let Some(deco) = opt_object_arg(cx, idx){
    let css = string_for_key(cx, &deco, "str")?;

    let line = string_for_key(cx, &deco, "line")?;
    let ty = match line.as_str(){
      "underline" => TextDecoration::UNDERLINE,
      "overline" => TextDecoration::OVERLINE,
      "line-through" => TextDecoration::LINE_THROUGH,
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

    let color = match string_for_key(cx, &deco, "color")?.as_str(){
      "currentColor" => None,
      color_str => css_to_color(&color_str),
    };

    let inherit = string_for_key(cx, &deco, "inherit")?;
    let size = match inherit.as_str(){
      "from-font" => None,
      _ => match opt_object_for_key(cx, &deco, "thickness"){
          Some(thickness) => {
            let raw_size = float_for_key(cx, &thickness, "size")?;
            let unit = string_for_key(cx, &thickness, "unit")?;
            let px_size = float_for_key(cx, &thickness, "px")?;
            Spacing::parse(raw_size, unit, px_size)
          }
          _ => None
        }
    };

    // if the setting is invalid, it should just be ignored
    if css.is_empty() || color.is_none(){ return Ok(None) }

    // As of skia_safe 0.78.2, `Gaps` mode is too buggy, with random breaks in places that don't have
    // descenders. It would be nice to enable this in a future release once it stabilizes…
    let mode = TextDecorationMode::Through;

    let decoration = Decoration{ ty, style, mode, ..Decoration::default() };
    Ok(Some(DecorationStyle{ decoration, size, color, css} ))
  }else{
    Ok(None)
  }
}


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
      "em" | "rem" => self.raw_size * em_size,
      _ => self.px_size
    }
  }

  pub fn to_string(&self) -> String{
    format!("{}{}", self.raw_size, self.unit)
  }
}

