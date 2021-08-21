#![allow(unused_variables)]
#![allow(unused_mut)]
#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(non_snake_case)]
use std::sync::{Mutex};
use std::fs;
use std::ops::Range;
use std::path::Path;
use std::collections::HashMap;
use neon::prelude::*;
use neon::result::Throw;

use skia_safe::{Font, FontMgr, FontMetrics, FontArguments, Typeface, Data, Paint, Point, Rect, Path as SkPath};
use skia_safe::font_style::{FontStyle, Weight, Width, Slant};
use skia_safe::font_arguments::{VariationPosition, variation_position::{Coordinate}};
use skia_safe::textlayout::{FontCollection, TypefaceFontProvider, TextStyle, TextAlign,
                            TextDirection, ParagraphStyle, Paragraph, ParagraphBuilder};

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
}

impl Typesetter{
  pub fn new(state:&State, text: &str, width:Option<f32>) -> Self {
    let mut library = FONT_LIBRARY.lock().unwrap();
    let (char_style, mut graf_style, baseline, wrap) = state.typography();
    let typefaces = library.collect_fonts(&char_style);
    let width = width.unwrap_or(GALLEY);
    let text = match wrap{
      true => text.to_string(),
      false => {
        graf_style.set_max_lines(1);
        text.replace("\n", " ")
      }
    };

    Typesetter{text, width, baseline, typefaces, char_style, graf_style}
  }

  pub fn layout(&self, paint:&Paint) -> (Paragraph, Point) {
    let mut char_style = self.char_style.clone();
    char_style.set_foreground_color(Some(paint.clone()));

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
      (rect.with_offset((alignment, offset)), range, baseline as f32)
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

  pub fn path(&mut self) -> Option<SkPath> {
    let families:Vec<String> = self.char_style.font_families().iter().map(|fam| fam.to_string()).collect();
    let matches = self.typefaces.find_typefaces(&families, self.char_style.font_style());
    if let Some(typeface) = matches.first(){
      let font = Font::from_typeface(typeface, self.char_style.font_size());
      let (leading, metrics) = font.metrics();
      let (width, bounds) = font.measure_str(&self.text, None);
      let offset = (
        width * get_alignment_factor(&self.graf_style),
        get_baseline_offset(&metrics, self.baseline)
      );

      Some(SkPath::from_str(&self.text, offset, &font))
    }else{
      None
    }
  }
}

//
// Font argument packing & unpacking
//

pub struct FontSpec{
  families: Vec<String>,
  size: f32,
  leading: f32,
  style: FontStyle,
  features: Vec<(String, i32)>,
  pub variant: String,
  pub canonical: String
}

pub fn font_arg(cx: &mut FunctionContext, idx: i32) -> Result<Option<FontSpec>, Throw> {
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

  let feat_obj = font_desc.get(cx, "features")?.downcast::<JsObject, _>(cx).or_throw(cx)?;
  let features = font_features(cx, &feat_obj)?;

  let style = FontStyle::new(weight, width, slant);
  Ok(Some(FontSpec{ families, size, leading, style, features, variant, canonical}))
}

pub fn font_features(cx: &mut FunctionContext, obj: &Handle<JsObject>) -> Result<Vec<(String, i32)>, Throw>{
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

#[derive(PartialEq, Eq, Hash)]
struct CollectionKey{ families:String, weight:i32, slant:Slant }

impl CollectionKey{
  pub fn new(style: &TextStyle) -> Self {
    let families = style.font_families();
    let families = families.iter().collect::<Vec<&str>>().join(", ");
    let weight = *style.font_style().weight();
    let slant = style.font_style().slant();
    CollectionKey{ families, weight, slant }
  }
}

//
// Font collection management
//

pub struct FontLibrary{
  pub fonts: Vec<(Typeface, Option<String>)>,
  pub collection: FontCollection,
  collection_cache: HashMap<CollectionKey, FontCollection>,
}

unsafe impl Send for FontLibrary {
  // famous last words: this ‘should’ be safe in practice because the singleton is behind a mutex
}

impl FontLibrary{
  pub fn shared() -> Mutex<Self>{
    let fonts = vec![];
    let collection_cache = HashMap::new();
    let mut collection = FontCollection::new();
    collection.set_default_font_manager(FontMgr::new(), None);
    Mutex::new(FontLibrary{ collection, collection_cache, fonts })
  }

  fn families(&self) -> Vec<String>{
    let font_mgr = FontMgr::new();
    let count = font_mgr.count_families();
    let mut names:Vec<String> = (0..count).map(|i| font_mgr.family_name(i)).collect();
    for (font, alias) in &self.fonts {
      names.push(match alias{
        Some(name) => name.clone(),
        None => font.family_name()
      })
    }
    names.sort();
    names.dedup();
    names
  }

  fn family_details(&self, family:&str) -> (Vec<f32>, Vec<String>, Vec<String>){
    // merge the system fonts and our dynamically added fonts into one list of FontStyles
    let mut dynamic = TypefaceFontProvider::new();
    for (font, alias) in &self.fonts{
      dynamic.register_typeface(font.clone(), alias.clone());
    }
    let std_mgr = FontMgr::new();
    let dyn_mgr:FontMgr = dynamic.into();
    let mut std_set = std_mgr.match_family(&family);
    let mut dyn_set = dyn_mgr.match_family(&family);
    let std_styles = (0..std_set.count()).map(|i| std_set.style(i));
    let dyn_styles = (0..dyn_set.count()).map(|i| dyn_set.style(i));
    let all_styles = std_styles.chain(dyn_styles);

    // set up a collection to query for variable fonts who specify their weights
    // via the 'wght' axis rather than through distinct files with different FontStyles
    let mut var_fc = FontCollection::new();
    var_fc.set_default_font_manager(FontMgr::new(), None);
    var_fc.set_asset_font_manager(Some(dyn_mgr));

    // pull style values out of each matching font
    let mut weights:Vec<i32> = vec![];
    let mut widths:Vec<String> = vec![];
    let mut styles:Vec<String> = vec![];
    all_styles.for_each(|(style, _name)| {
      widths.push(from_width(style.width()));
      styles.push(from_slant(style.slant()));
      weights.push(*style.weight());
      if let Some(font) = var_fc.find_typefaces(&[&family], style).first(){
        // for variable fonts, report all the 100× sizes they support within their wght range
        weights.append(&mut typeface_wght_range(&font));
      }
    });

    // repackage collected values
    widths.sort_by(|a, b| a.replace("normal", "_").partial_cmp(&b.replace("normal", "_")).unwrap());
    widths.dedup();
    styles.sort_by(|a, b| a.replace("normal", "_").partial_cmp(&b.replace("normal", "_")).unwrap());
    styles.dedup();
    weights.sort_unstable();
    weights.dedup();
    let weights = weights.iter().map(|w| *w as f32 ).collect();
    (weights, widths, styles)
  }

  fn add_typeface(&mut self, font:Typeface, alias:Option<String>){
    self.fonts.push((font, alias));

    let mut assets = TypefaceFontProvider::new();
    for (font, alias) in &self.fonts {
      assets.register_typeface(font.clone(), alias.as_ref());
    }

    self.collection.set_asset_font_manager(Some(assets.into()));
    self.collection_cache.drain();
  }

  pub fn update_style(&mut self, orig_style:&TextStyle, spec: &FontSpec) -> Option<TextStyle>{
    let mut style = orig_style.clone();

    // don't update the style if no usable family names were specified
    let matches = self.collection.find_typefaces(&spec.families, spec.style);
    if matches.is_empty(){
      return None
    }

    style.set_font_style(spec.style);
    style.set_font_families(&spec.families);
    style.set_font_size(spec.size);
    style.set_height(spec.leading / spec.size);
    style.set_height_override(true);
    style.reset_font_features();
    for (feat, val) in &spec.features{
      style.add_font_feature(feat, *val);
    }
    Some(style)
  }

  pub fn update_features(&mut self, orig_style:&TextStyle, features: &[(String, i32)]) -> TextStyle{
    let mut style = orig_style.clone();
    for (feat, val) in features{
      style.add_font_feature(feat, *val);
    }
    style
  }

  pub fn collect_fonts(&mut self, style: &TextStyle) -> FontCollection {
    let families = style.font_families();
    let families:Vec<&str> = families.iter().collect();
    let matches = self.collection.find_typefaces(&families, style.font_style());

    // if the matched typeface is a variable font, create an instance that matches
    // the current weight settings and return early with a new FontCollection that
    // contains just that single font instance
    if let Some(font) = matches.first() {
      if let Some(params) = font.variation_design_parameters(){

        // memoize the generation of single-weight FontCollections for variable fonts
        let key = CollectionKey::new(&style);
        if let Some(collection) = self.collection_cache.get(&key){
          return collection.clone()
        }

        // reconnect to the user-specified family name (if provided)
        let alias = self.fonts.iter().find_map(|(face, alias)|
          if Typeface::equal(font, face){ alias.clone() }else{ None }
        );

        for param in params {
          let chars = vec![param.tag.a(), param.tag.b(), param.tag.c(), param.tag.d()];
          let tag = String::from_utf8(chars).unwrap();
          if tag == "wght"{
            // NB: currently setting the value to *one less* than what was requested
            //     to work around weird Skia behavior that returns something nonlinearly
            //     weighted in many cases (but not for ±1 of that value). This makes it so
            //     that n × 100 values will render correctly (and the bug will manifest at
            //     n × 100 + 1 instead)
            let weight = *style.font_style().weight() - 1;
            let value = (weight as f32).max(param.min).min(param.max);
            let coords = [ Coordinate { axis: param.tag, value } ];
            let v_pos = VariationPosition { coordinates: &coords };
            let args = FontArguments::new().set_variation_design_position(v_pos);
            let face = font.clone_with_arguments(&args).unwrap();

            let mut dynamic = TypefaceFontProvider::new();
            dynamic.register_typeface(face, alias);

            let mut collection = FontCollection::new();
            collection.set_default_font_manager(FontMgr::new(), None);
            collection.set_asset_font_manager(Some(dynamic.into()));
            self.collection_cache.insert(key, collection.clone());
            return collection
          }
        }
      }
    }

    // if the matched font wasn't variable, then just return the standard collection
    self.collection.clone()
  }

}

//
// Javascript Methods
//

pub fn get_families(mut cx: FunctionContext) -> JsResult<JsArray> {
  let library = FONT_LIBRARY.lock().unwrap();
  let families = library.families();
  let names = strings_to_array(&mut cx, &families)?;
  Ok(names)
}

pub fn has(mut cx: FunctionContext) -> JsResult<JsBoolean> {
  let library = FONT_LIBRARY.lock().unwrap();
  let family = string_arg(&mut cx, 1, "familyName")?;
  let found = library.families().contains(&family);
  Ok(cx.boolean(found))
}

pub fn family(mut cx: FunctionContext) -> JsResult<JsValue> {
  let library = FONT_LIBRARY.lock().unwrap();
  let family = string_arg(&mut cx, 1, "familyName")?;
  let (weights, widths, styles) = library.family_details(&family);

  if weights.is_empty() {
    return Ok(cx.undefined().upcast())
  }

  let name = cx.string(family);
  let weights = floats_to_array(&mut cx, &weights)?;
  let widths = strings_to_array(&mut cx, &widths)?;
  let styles = strings_to_array(&mut cx, &styles)?;

  let details = JsObject::new(&mut cx);
  let attr = cx.string("family"); details.set(&mut cx, attr, name)?;
  let attr = cx.string("weights"); details.set(&mut cx, attr, weights)?;
  let attr = cx.string("widths"); details.set(&mut cx, attr, widths)?;
  let attr = cx.string("styles"); details.set(&mut cx, attr, styles)?;

  Ok(details.upcast())
}

pub fn addFamily(mut cx: FunctionContext) -> JsResult<JsValue> {
  let alias = opt_string_arg(&mut cx, 1);
  let filenames = cx.argument::<JsArray>(2)?.to_vec(&mut cx)?;
  let results = JsArray::new(&mut cx, filenames.len() as u32);

  for (i, filename) in strings_in(&mut cx, &filenames).iter().enumerate(){
    let path = Path::new(&filename);
    let typeface = match fs::read(path){
      Err(why) => {
        return cx.throw_error(format!("{}: \"{}\"", why, path.display()))
      },
      Ok(bytes) => Typeface::from_data(Data::new_copy(&bytes), None)
    };

    match typeface {
      Some(font) => {
        // add family/weight/width/slant details to return value
        let details = typeface_details(&mut cx, &filename, &font, alias.clone())?;
        results.set(&mut cx, i as u32, details)?;

        // register the typeface
        let mut library = FONT_LIBRARY.lock().unwrap();
        library.add_typeface(font, alias.clone());
      },
      None => {
        return cx.throw_error(format!("Could not decode font data in {}", path.display()))
      }
    }
  }

  Ok(results.upcast())
}

