//
// OpenType font tables, capabilities, & per-context variation/feature settings
//

use std::collections::HashMap;
use neon::prelude::*;
use allsorts::{binary::read::ReadScope, get_name::fontcode_get_name, tables::NameTable};
use read_fonts::{FontData, TableProvider, TopLevelTable, types::{Tag, Fixed, F2Dot14}, tables::{hhea::Hhea, os2::Os2, base::Base, fvar::Fvar, avar::Avar, mvar::Mvar, gsub::Gsub, gpos::Gpos, layout::FeatureParams}};
use skia_safe::{Data, FontArguments, FourByteTag, Typeface};
use skia_safe::font_style::Slant;
use skia_safe::font_arguments::{VariationPosition, variation_position::Coordinate};
use skia_safe::textlayout::TextStyle;

use crate::bridge::*;

// a subset of a typeface's tables, copied out of Skia once so read-fonts can parse them in place
// (as a TableProvider: `tables.hhea()`, `tables.gsub()`, etc.)
pub struct FontTables(Vec<(Tag, Data)>);

impl FontTables{
  // extract the requested tables (if present)
  fn from_face(face:Option<&Typeface>, tags:&[Tag]) -> Self{
    FontTables(tags.iter().filter_map(|&tag| Some((tag, face?.copy_table_data(u32::from_be_bytes(tag.to_be_bytes()))?))).collect())
  }


  // tables for BaselineMetrics
  pub fn metrics(face:Option<&Typeface>) -> Self{
    // skip fonts without a usable units-per-em (their values are in font units)
    let face = face.filter(|face| face.units_per_em().is_some_and(|upm| upm > 0));
    Self::from_face(face, &[Mvar::TAG, Fvar::TAG, Avar::TAG, Hhea::TAG, Os2::TAG, Base::TAG])
  }

  // tables for FontCapabilities
  pub fn capabilities(face:&Typeface) -> Self{
    Self::from_face(Some(face), &[Fvar::TAG, Gsub::TAG, Gpos::TAG])
  }

  // the `MVAR` deltas for the given metric tags at a variation position (normalized via `fvar` & `avar`)
  pub fn metric_deltas<const N:usize>(&self, tags:[Tag; N], position:&[Coordinate]) -> Option<[f64; N]>{
    let mvar = self.mvar().ok()?;
    let axes = self.fvar().ok()?.axes().ok()?;
    let avar = self.avar().ok(); // optional: without it, the normalized coordinates are used as-is
    let coords:Vec<F2Dot14> = axes.iter().enumerate().map(|(i, axis)| {
      let value = match position.iter().find(|coord| Tag::from_u32(*coord.axis) == axis.axis_tag()) {
        Some(coord) => Fixed::from_f64(coord.value as f64),
        None => axis.default_value(),
      };
      let coord = axis.normalize(value);
      match avar.as_ref().and_then(|avar| avar.axis_segment_maps().get(i)?.ok()) {
        Some(map) => map.apply(coord).to_f2dot14(),
        None => coord.to_f2dot14(),
      }
    }).collect();
    Some(tags.map(|tag| mvar.metric_delta(tag, &coords).map_or(0.0, |delta| delta.to_f64())))
  }

  // the `BASE` table's baseline coordinates for the `DFLT` script (in font units)
  pub fn base_coords<const N:usize>(&self, tags:[&[u8; 4]; N]) -> Option<[Option<f64>; N]>{
    let axis = self.base().ok()?.horiz_axis()?.ok()?;
    let (list, scripts) = (axis.base_tag_list()?.ok()?, axis.base_script_list().ok()?);
    let values = scripts.base_script_records().iter()
      .find(|record| record.base_script_tag() == *b"DFLT")?
      .base_script(scripts.offset_data()).ok()?
      .base_values()?.ok()?;
    Some(tags.map(|tag| {
      let index = list.baseline_tags().iter().position(|t| t.get() == *tag)?;
      Some(values.base_coords().get(index).ok()?.coordinate() as f64)
    }))
  }
}

impl<'a> TableProvider<'a> for &'a FontTables{
  fn data_for_tag(&self, tag:Tag) -> Option<FontData<'a>>{
    let tables:&'a FontTables = self;
    tables.0.iter().find(|(t, _)| *t == tag).map(|(_, data)| FontData::new(data))
  }
}

//
// Font capabilities (variation axes, OpenType features, & named instances)
//

#[derive(Debug, Clone)]
pub struct AxisDetails{
  pub tag: FourByteTag,
  pub min: f32,
  pub max: f32,
  pub def: f32,
  pub label: Option<String>,
}

#[derive(Debug, Clone)]
pub struct InstanceDetails{
  pub name: String,
  pub coords: Vec<(FourByteTag, f64)>,
  pub face: Typeface,
}

impl InstanceDetails{
  pub fn style(&self) -> Slant{ self.face.font_style().slant() }
}

#[derive(Debug, Clone)]
pub struct FeatureDetails{
  pub tag: String,
  pub label: Option<String>,
}

#[derive(Debug, Default)]
pub struct FontCapabilities{
  pub axes: Vec<AxisDetails>, // variable-font design axes (in fvar order)
  pub features: Vec<FeatureDetails>, // OpenType features (from GSUB/GPOS tables)
  pub instances: Vec<InstanceDetails>, // named instances (normal→italic→oblique, then in fvar order)
}

impl FontCapabilities{
  pub fn new(font:&Typeface) -> Self{
    // read the font's `name` table (for accessing axis, feature, and instance names)
    let mut names:HashMap<u16, String> = HashMap::new();
    if let Some(data) = font.copy_table_data(*FourByteTag::from_chars('n','a','m','e')){
      let bytes = data.as_bytes();
      if let Ok(table) = ReadScope::new(bytes).read::<NameTable<'_>>(){
        for record in table.name_records.iter(){
          // skip record 0 since that's also the no-label-for-this-feature ID
          if record.name_id == 0 || names.contains_key(&record.name_id){ continue }

          // use fontcode_get_name so it weights by unicode-coverage of the platform+encoding
          if let Some(name) = fontcode_get_name(bytes, record.name_id).ok().flatten()
            .and_then(|name| name.into_string().ok())
            .filter(|name| !name.is_empty()){
              names.insert(record.name_id, name);
          }
        }
      }
    }

    // walk the font's variation-axis table and build a tag -> human-readable label mapping
    let tables = &FontTables::capabilities(font);
    let fvar_axes:Vec<(FourByteTag, Option<String>)> = tables.fvar()
      .and_then(|fvar| fvar.axes()).unwrap_or_default().iter()
      .map(|axis| (
        FourByteTag::from(u32::from_be_bytes(axis.axis_tag().to_be_bytes())),
        names.get(&axis.axis_name_id().to_u16()).cloned(),
      ))
      .collect();

    // extract each axis's value-range and merge in the labels (preserving the fvar table's ordering)
    let mut axes = vec![];
    if let Some(params) = font.variation_design_parameters(){
      for param in params {
        let label = fvar_axes.iter().find(|(tag, _)| *tag == param.tag).and_then(|(_, label)| label.clone());
        axes.push(AxisDetails{ tag:param.tag, min:param.min, max:param.max, def:param.def, label });
      }
    }

    // extract supported feature tags from the GSUB+GPOS tables and pair with their human-readable labels
    let mut features:Vec<FeatureDetails> = vec![];
    let feature_lists = [tables.gsub().and_then(|gsub| gsub.feature_list()), tables.gpos().and_then(|gpos| gpos.feature_list())];
    for list in feature_lists.into_iter().flatten(){
      for record in list.feature_records(){
        let Ok(tag) = std::str::from_utf8(&record.feature_tag().to_be_bytes()).map(|tag| tag.trim_end().to_string()) else { continue };

        // most features use the 0 "no label" ID (since they have canonical names), but check for custom labels in
        // the FeatureParams for stylistic sets (ss01–ss20) and character variants (cv01–cv99)
        let numbered = tag.len() == 4 && (tag.starts_with("ss") || tag.starts_with("cv")) && tag[2..].bytes().all(|b| b.is_ascii_digit());
        let params = if numbered { record.feature(list.offset_data()).ok().and_then(|feature| feature.feature_params()?.ok()) } else { None };
        let name_id = match params {
          Some(FeatureParams::StylisticSet(ss)) => ss.ui_name_id().to_u16(),
          Some(FeatureParams::CharacterVariant(cv)) => cv.feat_ui_label_name_id().to_u16(),
          _ => 0,
        };

        // keep the first label either table provided for a tag
        let label = names.get(&name_id).cloned();
        match features.iter_mut().find(|existing| existing.tag == tag){
          Some(existing) => { if existing.label.is_none(){ existing.label = label; } }
          None => features.push(FeatureDetails{ tag, label }),
        }
      }
    }
    features.sort_by(|a, b| a.tag.cmp(&b.tag));

    // collect the face's named instances
    let mut instances:Vec<InstanceDetails> = vec![];
    if let Ok(fvar) = tables.fvar(){
      for record in fvar.instances().into_iter().flat_map(|records| records.iter()).flatten(){
        // skip unnamed and repeated instances
        let Some(name) = names.get(&record.subfamily_name_id.to_u16()) else { continue };
        if instances.iter().any(|inst| inst.name == *name){ continue }

        let coords = fvar_axes.iter().zip(record.coordinates).map(|((tag, _), val)| {
          // use the shortest decimal that rounds back to the same fixed-point value
          let val = val.get();
          let short = [1.0, 1e1, 1e2, 1e3, 1e4, 1e5].iter()
            .map(|scale| (val.to_f64() * scale).round() / scale)
            .find(|approx| Fixed::from_f64(*approx) == val);
          (*tag, short.unwrap_or(val.to_f64()))
        }).collect();
        instances.push(InstanceDetails{ name:name.clone(), coords, face:font.clone() });
      }
    }

    FontCapabilities{ axes, features, instances }
  }

  // expand range to the union of the two sets of axes+features+instances (so a family of faces can be summarized)
  pub fn merge(&mut self, other:Self){
    for axis in other.axes {
      match self.axes.iter_mut().find(|existing| existing.tag == axis.tag){
        Some(existing) => {
          existing.min = existing.min.min(axis.min);
          existing.max = existing.max.max(axis.max);
          if existing.label.is_none(){ existing.label = axis.label; }
        }
        None => self.axes.push(axis),
      }
    }

    for feature in other.features {
      match self.features.iter_mut().find(|existing| existing.tag == feature.tag){
        Some(existing) => { if existing.label.is_none(){ existing.label = feature.label; } }
        None => self.features.push(feature),
      }
    }
    self.features.sort_by(|a, b| a.tag.cmp(&b.tag));

    for instance in other.instances {
      // the first face to define a given instance name wins
      if !self.instances.iter().any(|existing| existing.name == instance.name){
        self.instances.push(instance);
      }
    }
    self.instances.sort_by_key(|inst| inst.style() as i32); // stable, so fvar order is kept within each face
  }

  // the min & max weights covered by the `wght` axis and all 100x steps in between
  pub fn wght_range(&self) -> Vec<i32>{
    let mut wghts = vec![];
    if let Some(axis) = self.axes.iter().find(|axis| axis.tag == Coordinate::wght){
      let (min, max) = (axis.min as i32, axis.max as i32);
      let mut val = min;
      while val <= max {
        wghts.push(val);
        val = val + 100 - (val % 100);
      }
      if !wghts.contains(&max){
        wghts.push(max);
      }
    }
    wghts
  }

  // return a js object mapping axis tags to {min, max, default, label} summaries
  pub fn variations_object<'a>(&self, cx: &mut FunctionContext<'a>) -> JsResult<'a, JsObject>{
    let dict = JsObject::new(cx);
    for axis in &self.axes {
      let range = JsObject::new(cx);
      range.prop(cx, "min")    .set(axis.min as f64)?;
      range.prop(cx, "max")    .set(axis.max as f64)?;
      range.prop(cx, "default").set(axis.def as f64)?;
      if let Some(label) = &axis.label{
        range.prop(cx, "label").set(label.as_str())?;
      }
      dict.prop(cx, String::from_utf8_lossy(&axis.tag.to_be_bytes()).as_ref()).set(range)?;
    }
    Ok(dict)
  }

  // return a js object mapping instance names to {style?, settings} objects
  pub fn instances_object<'a>(&self, cx: &mut FunctionContext<'a>, with_style:bool) -> JsResult<'a, JsObject>{
    let dict = JsObject::new(cx);
    for instance in &self.instances {
      let entry = JsObject::new(cx);
      if with_style{ entry.prop(cx, "style").set(from_slant(instance.style()))?; }
      let settings = JsObject::new(cx);
      for (tag, value) in &instance.coords {
        settings.prop(cx, String::from_utf8_lossy(&tag.to_be_bytes()).as_ref()).set(*value)?;
      }
      entry.prop(cx, "settings").set(settings)?;
      dict.prop(cx, instance.name.as_str()).set(entry)?;
    }
    Ok(dict)
  }

  // return a js object mapping feature tags to font-provided labels
  pub fn features_object<'a>(&self, cx: &mut FunctionContext<'a>) -> JsResult<'a, JsObject>{
    let dict = JsObject::new(cx);
    for feature in &self.features {
      dict.prop(cx, feature.tag.as_str()).set(feature.label.as_deref().unwrap_or(""))?;
    }
    Ok(dict)
  }
}

//
// Active variation-axis/feature settings
//

#[derive(Clone, Debug, Default)]
pub struct FontAxes{
  instance: Option<InstanceDetails>, // selected fontInstance (if any)
  variations: Vec<(String, f32)>, // decoded fontVariationSettings (overrides any instance settings)
}

impl FontAxes{
  pub fn set_instance(&mut self, instance:Option<InstanceDetails>){
    self.instance = instance;
  }

  pub fn instance(&self) -> Option<&str>{
    self.instance.as_ref().map(|inst| inst.name.as_str())
  }

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

  pub fn clear(&mut self){
    self.variations.clear();
    self.instance = None;
  }

  // merge the variable font instancing fields into the provided char_style
  pub fn apply(&self, char_style:&mut TextStyle, stretch:f32, oblique:Option<f32>){
    // make sure the font style will resolve to the face that contains the selected fontInstance (if any)
    if let Some(inst) = &self.instance{
      if !char_style.typeface().is_some_and(|face| face.unique_id() == inst.face.unique_id()){
        char_style.set_font_style(inst.face.font_style());
        char_style.set_typeface(inst.face.clone());
      }
    }

    let font_style = char_style.font_style();
    let axes = char_style.typeface().and_then(|tf| tf.variation_design_parameters());
    let has_axis = |tag| axes.as_ref().is_some_and(|params| params.iter().any(|p| p.tag == tag));
    let slant_only_font = has_axis(Coordinate::slnt) && !has_axis(Coordinate::ital);

    let mut coords = vec![
      Coordinate{ axis:Coordinate::wght, value:*font_style.weight() as f32 },
      Coordinate{ axis:Coordinate::wdth, value:stretch },
      Coordinate{ axis:Coordinate::opsz, value:char_style.font_size() },
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

    // a named instance overrides any style-derived values that collide
    if let Some(inst) = &self.instance{
      coords.extend(inst.coords.iter().map(|&(axis, value)| Coordinate{ axis, value:value as f32 }));
    }

    // fontVariationSettings still override everything else
    for (tag, value) in &self.variations{
      let b = tag.as_bytes();
      let axis = FourByteTag::from_chars(b[0] as char, b[1] as char, b[2] as char, b[3] as char);
      coords.push(Coordinate{ axis, value:*value });
    }

    // only set coordinates for axes the typeface supports and clamp them to the font's min/max range
    // (prevents cache fragmentation since the axis values will be part of a skia-internal cache key)
    coords.retain_mut(|coord|
      match axes.as_ref().and_then(|params| params.iter().find(|p| p.tag == coord.axis)){
        Some(axis) => { coord.value = coord.value.clamp(axis.min, axis.max); true }
        None => false
      }
    );
    if coords.is_empty(){ return }

    let args = FontArguments::new()
      .set_variation_design_position(VariationPosition{ coordinates: &coords });
    char_style.set_font_arguments(&args);
  }

  // state the measureText cache depends on
  pub fn cache_key(&self) -> (Vec<(String, u32)>, Option<String>){
    let variations = self.variations.iter().map(|(tag, value)| (tag.clone(), value.to_bits())).collect();
    let instances = self.instance().map(|name| name.to_lowercase());
    (variations, instances)
  }
}

#[derive(Clone, Debug, Default)]
pub struct FontFeatures{
  named:  Vec<(String, i32)>, // decoded from fontVariant keywords
  tagged: Vec<(String, i32)>, // raw 4-character tags from fontFeatureSettings
}

impl FontFeatures{
  pub fn set_named(&mut self, features:&[(String, i32)]){
    self.named = features.to_vec();
  }

  pub fn set_tagged(&mut self, mut tagged:Vec<(String, i32)>){
    tagged.sort_by(|(a, _), (b, _)| a.cmp(b));
    self.tagged = tagged;
  }

  pub fn clear_tagged(&mut self){
    self.tagged.clear();
  }

  // reconstruct the canonical css serialization
  pub fn css_string(&self) -> String{
    if self.tagged.is_empty(){ return "normal".to_string() }
    self.tagged.iter()
      .map(|(tag, value)| format!("\"{}\" {}", tag, value))
      .collect::<Vec<_>>()
      .join(", ")
  }

  // merge the keyword- and tag-based feature settings into the provided char_style
  pub fn apply(&self, char_style:&mut TextStyle, kerning_off:bool){
    for (feat, val) in &self.named{ char_style.add_font_feature(feat, *val); } // fontVariant
    if kerning_off{ char_style.add_font_feature("kern", 0); } // fontKerning
    for (feat, val) in &self.tagged{ char_style.add_font_feature(feat, *val); } // fontFeatureSettings
  }

  // state the measureText cache depends on
  pub fn cache_key(&self) -> Vec<(String, i32)>{
    self.named.iter().chain(self.tagged.iter()).cloned().collect()
  }
}
