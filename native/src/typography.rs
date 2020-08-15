// #![allow(unused_variables)]
// #![allow(unused_mut)]
// #![allow(dead_code)]
// #![allow(unused_imports)]
use std::rc::Rc;
use std::cell::RefCell;
use neon::prelude::*;
use neon::result::Throw;
use neon::object::This;
use skia_safe::{
  FontMetrics, FontArguments,
  font_style::{FontStyle, Weight, Width, Slant},
  font_arguments::{VariationPosition, variation_position::{Coordinate}}
};

use skia_safe::{FontMgr, Typeface, Data, textlayout::{FontCollection, TypefaceFontProvider, TextStyle}};
use skia_safe::textlayout::{TextAlign, TextDirection, ParagraphStyle};
use std::collections::HashMap;

use crate::utils::*;

pub struct FontSpec{
  families: Vec<String>,
  size: f32,
  leading: f32,
  style: FontStyle,
  features: Vec<(String, i32)>,
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
  let size = float_for_key(cx, &font_desc, "size")?;
  let leading = float_for_key(cx, &font_desc, "lineHeight")?;
  let weight = Weight::from(float_for_key(cx, &font_desc, "weight")? as i32);

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
  Ok(Some(FontSpec{ families, size, leading, style, features, variant, canonical}))
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

#[derive(PartialEq, Eq, Hash)]
struct CollectionKey{ families:String, weight:i32 }

pub struct FontLibrary{
  pub fonts: Vec<Typeface>,
  pub collection: FontCollection,
  collection_cache: HashMap<CollectionKey, FontCollection>,
}

impl Default for FontLibrary{
  fn default() -> Self{
    let mut library = FontCollection::new();
    library.set_default_font_manager(FontMgr::new(), None);
    FontLibrary{ collection: library, collection_cache:HashMap::new(), fonts:vec![] }
  }
}

impl FontLibrary{
  fn families(&self) -> Vec<String>{
    let font_mgr = FontMgr::new();
    let count = font_mgr.count_families();
    let mut names:Vec<String> = (0..count).map(|i| font_mgr.family_name(i)).collect();
    for font in &self.fonts {
      names.push(font.family_name());
    }
    names.sort();
    names
  }

  fn weights(&self, family: &str) -> Vec<f32> {
    // TKTK: look through self.fonts as well
    let font_mgr = FontMgr::new();
    let mut style_set = font_mgr.match_family(&family);

    let mut weights:Vec<i32> = (0..style_set.count()).map(|i| {
      let (style, _name) = style_set.style(i);
      *style.weight()
    }).collect();
    weights.sort();
    weights.dedup();
    weights.iter().map(|w| *w as f32 ).collect()
  }

  fn add_typeface(&mut self, font:Typeface){
    self.fonts.push(font);

    let mut assets = TypefaceFontProvider::new();
    for font in &self.fonts {
      let alias = font.family_name();
      assets.register_typeface(font.clone(), Some(&alias));
    }

    self.collection.set_asset_font_manager(Some(assets.into()));
    self.collection_cache = HashMap::new()
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

  pub fn with_style(&mut self, style: &TextStyle) -> FontCollection {
    let families = style.font_families();
    let families:Vec<&str> = families.iter().collect();

    // memoize the generation of single-weight FontCollections for variable fonts
    let key = CollectionKey{ families:families.join(", "), weight: *style.font_style().weight() };
    if let Some(collection) = self.collection_cache.get(&key){
      return collection.clone()
    }

    let matches = self.collection.find_typefaces(&families, style.font_style());
    if let Some(font) = matches.first() {
      let family = font.family_name();

      // if the matched typeface is a variable font, create an instance that matches
      // the current weight settings and return early with a new FontCollection that
      // contains just that single font instance
      if let Some(params) = font.variation_design_parameters(){
        for param in params {
          let chars = vec![param.tag.a(), param.tag.b(), param.tag.c(), param.tag.d()];
          let tag = String::from_utf8(chars).unwrap();
          if tag == "wght"{
            // NB: currently setting the value to *one less* than what was requested
            //     to work around weird Skia behavior that returns something too light
            //     in many cases (but not for ±1 of that value). This makes it so that
            //     n × 100 values will render correctly (and the bug will manifest at
            //     n × 100 + 1 instead)
            let weight = *style.font_style().weight() - 1;
            let value = (weight as f32).max(param.min).min(param.max);
            let coords = [ Coordinate { axis: param.tag, value } ];
            let v_pos = VariationPosition { coordinates: &coords };
            let args = FontArguments::new().set_variation_design_position(v_pos);
            let face = font.clone_with_arguments(&args).unwrap();

            let mut dynamic = TypefaceFontProvider::new();
            dynamic.register_typeface(face, Some(&family));

            let mut collection = FontCollection::new();
            collection.set_default_font_manager(FontMgr::new(), None);
            collection.set_asset_font_manager(Some(dynamic.into()));
            self.collection_cache.insert(key, collection.clone());
            return collection
          }
        }
      }
    }else{
      // TKTKTKTK: do something in the no-matches case
      // (maybe try subbing in concrete family names for the generic names?)
    }

    // if the matched font wasn't variable, then just return the standard collection
    self.collection.clone()
  }

}

// in practice the FontLibrary will always be a singleton, so base the js object
// on a refcell that can be borrowed by all the Context2Ds

pub struct SharedFontLibrary{
  pub library:Rc<RefCell<FontLibrary>>
}

impl Default for SharedFontLibrary{
  fn default() -> Self{
    SharedFontLibrary{ library: Rc::new(RefCell::new(FontLibrary::default())) }
  }
}

declare_types! {
  pub class JsFontLibrary for SharedFontLibrary {
    init(_) {
      Ok( SharedFontLibrary::default() )
    }

    method get_families(mut cx){
      let this = cx.this();
      let families = cx.borrow(&this, |this| {
        let library = this.library.borrow_mut();
        library.families()
      });
      Ok(strings_to_array(&mut cx, &families)?)
    }

    method family(mut cx){
      let this = cx.this();
      let family = cx.argument::<JsString>(0)?.value();
      let weights = cx.borrow(&this, |this| {
        let library = this.library.borrow_mut();
        library.weights(&family)
      });

      Ok(floats_to_array(&mut cx, &weights)?)
    }

    method useFont(mut cx){
      let this = cx.this();
      let buffer = cx.argument::<JsBuffer>(0)?;
      match cx.borrow(&buffer, |buf_data| {
        Typeface::from_data(Data::new_copy(buf_data.as_slice()), None)
      }){
        Some(font) => {
          cx.borrow(&this, |this| {
            let mut library = this.library.borrow_mut();
            library.add_typeface(font);
          });
          Ok(cx.undefined().upcast())
        },
        None => cx.throw_error("Could not decode font data")
      }
    }

  }
}

