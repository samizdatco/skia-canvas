#![allow(dead_code)]
#![allow(non_snake_case)]
use std::ops::Range;
use std::iter::zip;
use neon::prelude::*;
use serde_json::{json, Value};
use skia_safe::{FontMetrics, Typeface, Paint, Point, Rect, Path as SkPath, Color};
use skia_safe::font_style::{FontStyle, Weight, Width, Slant};
use skia_safe::textlayout::{
  Decoration, FontCollection, Paragraph, ParagraphBuilder, ParagraphStyle, RectHeightStyle, RectWidthStyle,
  TextAlign, TextDecoration, TextDecorationMode, TextDecorationStyle, TextDirection, TextStyle,
};
use crate::font_library::FontLibrary;
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
  text_wrap: bool,
}

impl Typesetter{
  pub fn new(state:&State, text: &str, width:Option<f32>) -> Self {
    let (char_style, graf_style, text_decoration, baseline, text_wrap) = state.typography();
    let typefaces = FontLibrary::with_shared(|lib|
      lib
        .set_hinting(graf_style.hinting_is_on())
        .collect_fonts(&char_style)
    );
    let width = width.unwrap_or(GALLEY);
    let text = match text_wrap{
      true => text.to_string(),
      false => text.replace("\n", " ")
    };

    Typesetter{text, width, baseline, typefaces, char_style, graf_style, text_decoration, text_wrap}
  }

  pub fn layout(&self, paint:&Paint) -> (Paragraph, Point) {
    let mut char_style = self.char_style.clone();
    char_style.set_foreground_paint(paint);
    char_style.set_decoration(
      &self.text_decoration.for_layout(&char_style, paint.color())
    );

    // prevent SkParagraph from faking the font style if the match isn't the requested weight/slant
    let fams:Vec<String> = char_style.font_families().iter().map(|s| s.to_string()).collect();
    if let Some(matched) = self.typefaces.clone().find_typefaces(&fams, char_style.font_style()).first(){
      char_style.set_font_style(matched.font_style());
    }

    let mut paragraph_builder = ParagraphBuilder::new(&self.graf_style, &self.typefaces);
    paragraph_builder.push_style(&char_style);
    paragraph_builder.add_text(&self.text);

    let mut paragraph = paragraph_builder.build();
    paragraph.layout(self.width);

    let offset = Point::new(
      self.alignment_offset(),
      -paragraph.alphabetic_baseline(),
    );

    (paragraph, offset)
  }

  pub fn metrics(&self) -> Value {
    let (mut paragraph, origin) = self.layout(&Paint::default());
    let mut line_rects:Vec<Rect> = vec![]; // accumulate line rects to calculate full bounds

    // calculate baseline offsets (relative to line_metrics.baseline which reflects ctx.textBaseline setting)
    let shift = self.char_style.baseline_shift();
    let hang = Baseline::Hanging.get_offset(&self.char_style) - shift;
    let norm = Baseline::Alphabetic.get_offset(&self.char_style) - shift;
    let ideo = Baseline::Ideographic.get_offset(&self.char_style) - shift;

    // calculate bounds for each single-font block of glyphs on each line (and gather font info)
    struct TextRun{ line: usize, family: String, metrics: FontMetrics, bounds: Rect }
    let mut text_runs:Vec<TextRun> = vec![];
    paragraph.extended_visit(|line, visit|{
      if let Some(info) = visit{
        text_runs.push(TextRun{
          line,
          family: info.font().typeface().family_name(),
          metrics: info.font().metrics().1,
          bounds: zip(info.positions(), info.bounds())
            .filter(|(_, rect)| !rect.is_empty())
            .map(|(pt, rect)| rect.with_offset(*pt + info.origin() + origin - Point::new(0.0, norm)))
            .reduce(Rect::join2)
            .unwrap_or(Rect::new_empty())
        });
      }
    });

    // measure each line and add its layout rect to `line_rects`
    let lines = (0..paragraph.line_number()).filter_map(|ln|{
      // find the range of byte & char indices that are on this line (includes trailing whitespace if not wrapping)
      let text_range = paragraph.get_actual_text_range(ln, !self.text_wrap);
      let char_range = utf16_range(&self.text, &text_range);

      // calculate this line's vertical offsets relative to the typesetting origin
      let line_metrics = paragraph.get_line_metrics_at(ln)?;
      let half_leading = self.graf_style.strut_style().leading().max(0.0) * self.char_style.font_size() / 2.0;
      let baseline = line_metrics.baseline as f32 + origin.y - half_leading;
      let line_ascent = baseline - line_metrics.ascent as f32;
      let line_descent = baseline + line_metrics.descent as f32;

      // combine the glyph bounds of all single-font runs on this line (potentially omitting trailing spaces)
      let font_runs = text_runs.iter().filter(|r| r.line==ln).collect::<Vec<&TextRun>>();
      let text_bounds = font_runs.iter()
        .map(|run| run.bounds)
        .reduce(Rect::join2)
        .unwrap_or(Rect::new_empty());

      // calculate horizontal line bounds that include trailing whitespace for use in `actualBoundingBox`
      // (and compensate for the extra half-letterspace added to the start & end of each line)
      line_rects.push(
        paragraph
          .get_rects_for_range(char_range.clone(), RectHeightStyle::Tight, RectWidthStyle::Tight).iter()
          .map(|tb| {
            let Rect{top, bottom, ..} = text_bounds;
            let Rect{left, right, ..} = tb.rect.with_offset(origin);
            Rect::new(left, top, right - self.char_style.letter_spacing(), bottom)
          })
          .reduce(Rect::join2)
          .unwrap_or(text_bounds)
      );

      Some(json!({
        "x": text_bounds.left,
        "y": text_bounds.top,
        "width": text_bounds.width(),
        "height": text_bounds.height(),
        "baseline": baseline, // corresponds to the ctx.textBaseline selection
        "hangingBaseline": baseline - hang,
        "alphabeticBaseline": baseline - norm,
        "ideographicBaseline": baseline - ideo,
        "ascent": line_ascent,
        "descent": line_descent,
        "startIndex": char_range.start,
        "endIndex": char_range.end,
        "runs": font_runs.iter().map(|TextRun{family, metrics, bounds, ..}| {
          json!({
            "x": bounds.left,
            "y": bounds.top,
            "width": bounds.width(),
            "height": bounds.height(),
            "family": family,
            "ascent": baseline - norm + metrics.ascent,
            "descent": baseline - norm + metrics.descent,
            "capHeight": baseline - norm - metrics.cap_height,
            "xHeight": baseline - norm - metrics.x_height,
            "underline": metrics.underline_position().map(|ulH| baseline - norm + ulH ),
            "strikethrough": metrics.strikeout_position().map(|stH| baseline - norm + stH ),
          })
        }).collect::<Vec<Value>>()
      }))
    }).collect::<Vec<Value>>();

    // combine all the individual line measurements to find the `actualBoundingBox`
    let full_bounds = line_rects.into_iter()
      .reduce(Rect::join2)
      .unwrap_or(Rect::new_empty());

    // use line metrics to find maximal ascent/descent of all fonts on first line
    let (ascent, descent) = paragraph.get_line_metrics_at(0).map(|line|
      (norm + line.ascent as f32, line.descent as f32 - norm)
    ).unwrap_or_else(||{
      // or fall back to the first-matched font's metrics if measuring empty string
      let FontMetrics{ascent, descent, ..} = self.char_style.font_metrics();
      (norm - ascent, descent - norm)
    });

    json!({
      "width": full_bounds.right - full_bounds.left,
      "actualBoundingBoxLeft": -full_bounds.left,
      "actualBoundingBoxRight": full_bounds.right,
      "actualBoundingBoxAscent": -full_bounds.top,
      "actualBoundingBoxDescent": full_bounds.bottom,
      "fontBoundingBoxAscent": ascent,
      "fontBoundingBoxDescent": descent,
      "emHeightAscent": ascent,
      "emHeightDescent": descent,
      "hangingBaseline": hang,
      "alphabeticBaseline": norm,
      "ideographicBaseline": ideo,
      "lines": lines,
    })
  }

  pub fn path(&mut self, point:impl Into<Point>) -> SkPath {
    let (mut paragraph, mut origin) = self.layout(&Paint::default());
    let headroom = self.char_style.font_metrics().ascent + paragraph.alphabetic_baseline();
    let offset = self.baseline.get_offset(&self.char_style);
    origin += point.into();
    origin.y -= headroom - offset;

    let mut path = SkPath::new();
    for idx in 0..paragraph.line_number(){
      let (_skipped, line) = paragraph.get_path_at(idx);
      path.add_path(&line, origin, None);
    };
    path
  }

  fn alignment_offset(&self) -> f32{
    // convert start/end to left/right depending on writing system
    let gravity = match (self.graf_style.text_direction(), self.graf_style.text_align()){
      (TextDirection::LTR, TextAlign::Start) | (TextDirection::RTL, TextAlign::End) => TextAlign::Left,
      (TextDirection::LTR, TextAlign::End) | (TextDirection::RTL, TextAlign::Start) => TextAlign::Right,
      (_, alignment) => alignment,
    };

    // `alignment_factor` shifts the entire line to left/right/center align it
    // `spacing_step` compensates for the letterspacing Paragraph adds before the line's first character
    let (alignment_factor, spacing_step) = match gravity{
      TextAlign::Left | TextAlign::Justify => (0.0, -0.5),
      TextAlign::Center => (-0.5, 0.5),
      TextAlign::Right => (-1.0, 1.0),
      _ => (0.0, 0.0) // start & end have already been remapped
    };

    alignment_factor * self.width + spacing_step * self.char_style.letter_spacing()
  }
}

//
// Convert utf-8 byte indices -> utf-16 codepoint indices
//
fn utf16_range(text:&str, byte_range:&Range<usize>) -> Range<usize>{
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
  let weight = Weight::from(float_for_key(cx, &font_desc, "weight")? as i32);
  let slant = to_slant(string_for_key(cx, &font_desc, "style")?.as_str());
  let width = to_width(string_for_key(cx, &font_desc, "stretch")?.as_str());
  let line_height = opt_float_for_key(cx, &font_desc, "lineHeight")
    .map(|pt_size| pt_size / size);

  let feat_obj:Handle<JsObject> = font_desc.get(cx, "features")?;
  let features = font_features(cx, &feat_obj)?;

  Ok(match families[0] == ""{
    true => None, // silently fail if a family name was omitted (e.g., "bold 50px")
    false => Some(FontSpec{ families, size, line_height, weight, slant, width, features, variant, canonical})
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

impl Baseline{
  pub fn get_offset(&self, style:&TextStyle) -> f32 {
    let FontMetrics{mut ascent, mut descent, ..} = style.font_metrics();
    ascent -= style.baseline_shift();  // offsets are defined relative to the alphabetic baseline, so
    descent -= style.baseline_shift(); // compensate for any other textBaseline setting

    // see TextMetrics::GetFontBaseline from Chromium for reference:
    // https://github.com/chromium/chromium/blob/main/third_party/blink/renderer/core/html/canvas/text_metrics.cc#L34
    match self {
      Baseline::Top => -ascent,
      Baseline::Hanging => -ascent * 0.8,
      Baseline::Middle => -(ascent + descent) / 2.0,
      Baseline::Alphabetic => 0.0,
      Baseline::Bottom | Baseline::Ideographic => -descent,
    }
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
        Some(thickness) => Spacing::from_obj(cx, &thickness)?,
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
      "em" | "rem" => self.raw_size * em_size,
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
