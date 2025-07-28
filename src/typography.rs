// #![allow(unused_variables)]
// #![allow(unused_mut)]
// #![allow(unused_imports)]
#![allow(dead_code)]
#![allow(non_snake_case)]
use std::ops::Range;
use neon::prelude::*;
use serde_json::{json, Value};
use skia_safe::{FontMetrics, Typeface, Paint, Point, Rect, Path as SkPath, Color};
use skia_safe::font_style::{FontStyle, Weight, Width, Slant};
use skia_safe::textlayout::{
  paragraph::FontInfo, Decoration, FontCollection, Paragraph, ParagraphBuilder, ParagraphStyle, RectHeightStyle, RectWidthStyle,
  TextAlign, TextDecoration, TextDecorationMode, TextDecorationStyle, TextDirection, TextStyle, TextBox,
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
    let (paragraph, origin) = self.layout(&Paint::default());
    let font_runs = paragraph.get_fonts(); // char ranges of contiguous fonts (and their metrics)
    let lookup = CharMap::new(&self.text, &paragraph); // the byte-idx -> glyph/char-idx mapping

    // calculate baseline positions (as offsets to origin based on current ctx.textBaseline setting)
    let shift = self.char_style.baseline_shift();
    let hang = Baseline::Hanging.get_offset(&self.char_style) - shift;
    let norm = Baseline::Alphabetic.get_offset(&self.char_style) - shift;
    let ideo = Baseline::Ideographic.get_offset(&self.char_style) - shift;

    // use line metrics to find maximal ascent/descent of all fonts on first line
    let (ascent, descent) = paragraph.get_line_metrics_at(0).map(|line|
      (norm + line.ascent as f32, line.descent as f32 - norm)
    ).unwrap_or_else(||{
      // or fall back to the first-matched font's metrics
      let FontMetrics{ascent, descent, ..} = self.char_style.font_metrics();
      (norm - ascent, descent - norm)
    });

    // adjust layout rects to line up with the origin and compensate for half-letterspacing at the start & end of the line
    let get_text_bounds = |tb:&TextBox| -> Rect{
      let Rect{left, top, right, bottom} = tb.rect;
      Rect::new(left, top, right - self.char_style.letter_spacing(), bottom).with_offset(origin)
    };

    // take the union of each line's bounds to construct the whole-text-run bounds
    let full_bounds:Rect = (0..paragraph.line_number())
      .flat_map(|ln| paragraph.get_rects_for_range(
        lookup.glyph_range(&paragraph.get_actual_text_range(ln, !self.text_wrap)), RectHeightStyle::Tight, RectWidthStyle::Tight
      ))
      .map(|tb| get_text_bounds(&tb))
      .reduce(Rect::join2)
      .unwrap_or(Rect::new_empty());

    json!({
      "width": (-full_bounds.left).max(0.0) + full_bounds.right,
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
      "lines": (0..paragraph.line_number()).filter_map(|ln|{
        // find the range of byte & char indices that are on this line (includes trailing whitespace if not wrapping)
        let text_range = paragraph.get_actual_text_range(ln, !self.text_wrap);
        let char_range = lookup.char_range(&text_range);

        // find layout sub-rectangles within this line and union them for the full-line bounds
        let line_bounds = paragraph
          .get_rects_for_range(lookup.glyph_range(&text_range), RectHeightStyle::Tight, RectWidthStyle::Tight)
          .iter()
          .map(get_text_bounds)
          .reduce(Rect::join2)
          .unwrap_or(Rect::new_empty());

        // calculate this line's baseline offset relative to the typesetting origin
        let line_metrics = paragraph.get_line_metrics_at(ln)?;
        let half_leading = self.graf_style.strut_style().leading().max(0.0) * self.char_style.font_size() / 2.0;
        let baseline = line_metrics.baseline as f32 + origin.y - half_leading;

        Some(json!({
          "x": line_bounds.left,
          "y": line_bounds.top,
          "width": line_bounds.width(),
          "height": line_bounds.height(),
          "baseline": baseline, // corresponds to the ctx.textBaseline selection
          "hangingBaseline": baseline - hang,
          "alphabeticBaseline": baseline - norm,
          "ideographicBaseline": baseline - ideo,
          "startIndex": char_range.start,
          "endIndex": char_range.end,
          "runs": font_runs.iter().filter_map(|FontInfo{text_range: font_range, font}|{
            // divide line into runs of contiguous font use, calculating layout dimensions and relative font metrics
            match font_range.start.max(text_range.start)..font_range.end.min(text_range.end){
              rng if !rng.is_empty() => Some(rng),
              _ => None
            }.and_then(|overlap|{
              let (_, metrics) = font.metrics();
              let glyph_range = lookup.glyph_range(&overlap);
              let base = baseline - norm;

              paragraph
                .get_rects_for_range(glyph_range, RectHeightStyle::Tight, RectWidthStyle::Tight)
                .first() // there should only ever be one rect within a single font run
                .map(|text_box|{
                  let rect = get_text_bounds(text_box);
                  json!({
                    "x": rect.left,
                    "y": rect.top,
                    "width": rect.width(),
                    "height": rect.height(),
                    "family": font.typeface().family_name(),
                    "ascent": base + metrics.ascent,
                    "descent": base + metrics.descent,
                    "capHeight": base - metrics.cap_height,
                    "xHeight": base - metrics.x_height,
                    "underline": metrics.underline_position().map(|ulH| base + ulH ),
                    "strikethrough": metrics.strikeout_position().map(|stH| base + stH ),
                  })
                })
            })
          }).collect::<Vec<Value>>()
        }))
      }).collect::<Vec<Value>>()
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
// Convert byte indices -> glyph or codepoint indices
//
struct CharMap{
  glyphs:Vec<Range<usize>>, // vec[glyph_index] -> byte_range
  chars:Vec<Range<usize>>, // vec[char_index] -> byte_index
}

impl CharMap{
  fn new(text:&str, graf:&Paragraph) -> Self{
    let mut glyphs:Vec<Range<usize>> = vec![];
    let mut at = 0;
    while at < text.len(){
      match graf.get_glyph_cluster_at(at){
        Some(cluster) => {
          at = cluster.text_range.end;
          glyphs.push(cluster.text_range)
        },
        None => break
      }
    }

    let mut chars:Vec<Range<usize>> = vec![];
    let mut indices = text.char_indices();
    loop{
      match indices.next(){
        Some((idx, _)) => chars.push(idx..indices.offset()),
        None => break,
      }
    }

    Self{glyphs, chars}
  }

  fn glyph_range(&self, byte_range:&Range<usize>) -> Range<usize>{
    let start = self.glyphs.iter().position(|g| g.start >= byte_range.start).unwrap_or(0);
    let end = self.glyphs.iter().rposition(|g| g.start < byte_range.end).map(|i| i + 1).unwrap_or(start);
    start..end
  }

  fn char_range(&self, byte_range:&Range<usize>) -> Range<usize>{
    let start = self.chars.iter().position(|c| c.start >= byte_range.start).unwrap_or(0);
    let end = self.chars.iter().rposition(|c| c.start < byte_range.end).map(|i| i + 1).unwrap_or(start);
    start..end
  }
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
