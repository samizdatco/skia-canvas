//
// OpenType font tables, capabilities, & per-context variation/feature settings
//

use std::collections::HashMap;
use neon::prelude::*;
use read_fonts::{FontData, FontRead, TableProvider, TopLevelTable, types::{Tag, Fixed, F2Dot14}, tables::{name::Name, hhea::Hhea, os2::Os2, base::Base, fvar::Fvar, avar::Avar, mvar::Mvar, gsub::Gsub, gpos::Gpos, head::Head, layout::FeatureParams}};
use skia_safe::{Data, FontArguments, FourByteTag, Typeface};
use skia_safe::font_style::{FontStyle, Weight, Slant};
use skia_safe::font_parameters::VariationAxis;
use skia_safe::font_arguments::{VariationPosition, variation_position::Coordinate};
use skia_safe::textlayout::TextStyle;

use crate::bridge::*;
use crate::font_library::FontLibrary;

pub struct FontTables(Vec<(Tag, Data)>);

impl FontTables{
  // extract the requested tables (if present)
  fn from_face(face:Option<&Typeface>, tags:&[Tag]) -> Self{
    FontTables(tags.iter().filter_map(|&tag| Some((tag, face?.copy_table_data(u32::from_be_bytes(tag.to_be_bytes()))?))).collect())
  }

  // uniquely identify the font file the typeface was derived from
  pub fn id(face:&Typeface) -> Option<Data>{
    face.copy_table_data(u32::from_be_bytes(Head::TAG.to_be_bytes()))
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
pub struct FeatureDetails{
  pub tag: String,
  pub label: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AxisDetails{
  pub tag: FourByteTag,
  pub min: f32,
  pub max: f32,
  pub def: f32,
  pub label: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NamedInstance{
  pub name: String, // the instance name (e.g., "Caption Semibold")
  pub coords: Vec<(FourByteTag, f64)>, // axis tag -> value mappings
  pub face: Typeface, // the variable font that this instance is derived from
}

impl NamedInstance{
  pub fn new(face:&Typeface, name:&str, coords:Vec<(FourByteTag, f64)>) -> Self{
    NamedInstance{ name:name.to_string(), coords, face:face.clone() }
  }

  pub fn style(&self) -> Slant{ self.face.font_style().slant() }
}

#[derive(Debug, Default)]
pub struct FontCapabilities{
  pub features: Vec<FeatureDetails>, // OpenType features (from GSUB/GPOS tables)
  pub axes: Vec<AxisDetails>, // variable-font design axes (in fvar order)
  pub instances: Vec<NamedInstance>, // font-defined name -> axis-settings bundles
}

impl FontCapabilities{
  pub fn new(font:&Typeface) -> Self{
    // read the font's `name` table (for accessing axis, feature, and instance names)
    let mut names:HashMap<u16, String> = HashMap::new();
    if let Some(data) = font.copy_table_data(*FourByteTag::from_chars('n','a','m','e')){
      if let Ok(table) = Name::read(FontData::new(data.as_bytes())){
        let mut scores:HashMap<u16, (bool, u8, u8)> = HashMap::new();
        for record in table.name_record(){
          let id = record.name_id().to_u16();

          // rank by unicode coverage (as in harfbuzz), avoiding lossy MacRoman strings
          let coverage = match (record.platform_id(), record.encoding_id()){
            (3, 10) => 10, (0, 6) => 9, (0, 4) => 8, (3, 1) => 7, (0, 3) => 6,
            (0, 2) => 5, (0, 1) => 4, (0, 0) => 3, (3, 0) => 2, (1, 0) => 1,
            _ => continue,
          };

          // then prefer English, followed by Unicode-platform strings
          let language = match (record.platform_id(), record.language_id()){
            (3, lang) if lang & 0xFF == 0x09 => 2, // any English LCID (en-US is 0x0409)
            (1, 0) => 2,
            (0, _) => 1,
            _ => 0,
          };
          let score = (coverage > 1, language, coverage);

          // skip record 0 since that's also the no-label-for-this-feature ID
          if id == 0 || scores.get(&id).is_some_and(|best| *best >= score){ continue }

          if let Some(name) = record.string(table.string_data()).ok()
            .map(|name| name.to_string())
            .filter(|name| !name.is_empty()){
              scores.insert(id, score);
              names.insert(id, name);
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
    let mut instances:Vec<NamedInstance> = vec![];
    if let Ok(fvar) = tables.fvar(){
      for record in fvar.instances().into_iter().flat_map(|records| records.iter()).flatten(){
        // skip unnamed and repeated instances
        let Some(name) = names.get(&record.subfamily_name_id.to_u16()) else { continue };
        if instances.iter().any(|inst| inst.name == *name){ continue }

        let coords:Vec<(FourByteTag, f64)> = fvar_axes.iter().zip(record.coordinates).map(|((tag, _), val)| {
          // fvar coords are 16.16 fixed-point, convert to an f64 that rounds back to the same value
          let val = val.get();
          let short = [1.0, 1e1, 1e2, 1e3, 1e4, 1e5].iter()
            .map(|scale| (val.to_f64() * scale).round() / scale)
            .find(|approx| Fixed::from_f64(*approx) == val);
          (*tag, short.unwrap_or(val.to_f64()))
        }).collect();
        instances.push(NamedInstance::new(font, name, coords));
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

#[derive(Clone, Debug)]
pub struct FontAxes{
  instance: Option<NamedInstance>, // selected fontInstance (if any)
  variations: Vec<(String, f32)>,  // decoded fontVariationSettings (overrides any instance settings)
  layout_style: Option<FontStyle>, // the style to request in layout (if the instance/variations differ from the font's)
  allows_synthesis: bool, // false when synthetic bold/italic would misrepresent the axis values
}

impl Default for FontAxes{
  fn default() -> Self{
    FontAxes{ instance:None, variations:vec![], layout_style:None, allows_synthesis:true }
  }
}

impl FontAxes{
  pub fn set_instance(&mut self, instance:Option<NamedInstance>){
    self.instance = instance;
  }

  pub fn instance(&self) -> Option<&str>{
    self.instance.as_ref().map(|inst| inst.name.as_str())
  }

  pub fn allows_synthesis(&self) -> bool{
    self.allows_synthesis
  }

  // update layout_style to match instance & variation settings (so fallback fonts match and synthetic bold/ital aren't
  // over-applied) unless it would cause skia to match a different face altogether
  pub fn resolve_style(&mut self, char_style:&TextStyle, stretch:f32, oblique:Option<f32>){
    let (font_face, mut collection) = FontLibrary::with_shared(|lib| (lib.font_face(char_style), lib.font_collection()));
    let (face, base) = match (&self.instance, font_face){
      (Some(inst), _) => (inst.face.clone(), inst.face.font_style()),
      (None, Some(face)) => (face, char_style.font_style()),
      (None, None) => { (self.layout_style, self.allows_synthesis) = (None, true); return }
    };

    // merge the instance's coords with explicit variation settings (only for axes the font actually has)
    let axes:Vec<VariationAxis> = face.variation_design_parameters().unwrap_or_default().into_iter()
      .filter(css_compatible) // skip legacy faces that don't use css value ranges
      .collect();
    let overrides:Vec<(FourByteTag, f32)> = self.instance.iter()
      .flat_map(|inst| inst.coords.iter().map(|&(tag, value)| (tag, value as f32)))
      .chain(self.variations.iter().map(|(tag, value)| (axis_tag(tag), *value)))
      .filter_map(|(tag, value)| axes.iter().find(|axis| axis.tag == tag).map(|axis| (tag, value.clamp(axis.min, axis.max))))
      .collect();

    // derive a FontStyle that matches the axis coords (to use for fallback fonts and synthesis decisions)
    let implied = {
      let value = |axis| overrides.iter().rev().find(|(tag, _)| *tag == axis).map(|(_, value)| *value);
      let weight = value(Coordinate::wght).map_or(base.weight(), |wght| Weight::from(wght as i32));
      let width = value(Coordinate::wdth).map_or(base.width(), nearest_width);
      let slant = match (value(Coordinate::ital), value(Coordinate::slnt)){
        (None, None) => base.slant(),
        (ital, slnt) => match ital.is_some_and(|ital| ital >= 0.5) || slnt.is_some_and(|slnt| slnt < 0.0){
          true => Slant::Italic,
          false => Slant::Upright,
        },
      };
      FontStyle::new(weight, width, slant)
    };

    // use the implied style so long as it doesn't cause skia to match to a different face
    (self.layout_style, self.allows_synthesis) = match self.instance.is_none() && implied == base{
      true => (None, true),
      false => {
        let families:Vec<String> = char_style.font_families().iter().map(|family| family.to_string()).collect();
        let selected = collection.find_typefaces(&families, implied).into_iter().next();
        match selected.is_some_and(|selected| FontTables::id(&selected) == FontTables::id(&face)){
          true => (Some(implied), true),
          false => (Some(base), false),
        }
      }
    };

    // disable synthesis if skia would fake a weight/slant that the axis settings are already providing
    let has_axis = |tag| axes.iter().any(|axis| axis.tag == tag);
    let (weight_axis, slant_axis) = (has_axis(Coordinate::wght), has_axis(Coordinate::ital) || has_axis(Coordinate::slnt));
    if self.allows_synthesis && (weight_axis || slant_axis){
      let mut scratch = char_style.clone();
      self.apply(scratch.set_typeface(face.clone()), stretch, oblique);
      let wanted = scratch.font_style();
      if let Some(drawn) = scratch.font_arguments().and_then(|args| args.clone_typeface(face)).map(|face| face.font_style()){
        let fakes_bold = weight_axis && *wanted.weight() >= 600 && *wanted.weight() - *drawn.weight() >= 200;
        let fakes_italic = slant_axis && wanted.slant() == Slant::Italic && drawn.slant() != Slant::Italic;
        self.allows_synthesis = !(fakes_bold || fakes_italic);
      }
    }
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
    *self = Self::default();
  }

  // merge the variable font instancing fields into the provided char_style
  pub fn apply(&self, char_style:&mut TextStyle, stretch:f32, oblique:Option<f32>){
    if let Some(style) = self.layout_style{
      char_style.set_font_style(style); // use the resolved style
    }
    if let Some(inst) = &self.instance{
      char_style.set_typeface(inst.face.clone()); // use the instance's 'parent' typeface
    }

    let font_style = char_style.font_style();
    let axes = char_style.typeface().and_then(|tf| tf.variation_design_parameters());
    let has_axis = |tag| axes.as_ref().is_some_and(|params| params.iter().any(|p| p.tag == tag));
    let slant_only_font = has_axis(Coordinate::slnt) && !has_axis(Coordinate::ital);

    // only set weight & stretch axes if they use css value ranges
    let mut coords:Vec<Coordinate> = [
      Coordinate{ axis:Coordinate::wght, value:*font_style.weight() as f32 },
      Coordinate{ axis:Coordinate::wdth, value:stretch },
      Coordinate{ axis:Coordinate::opsz, value:char_style.font_size() },
    ].into_iter()
      .filter(|coord| axes.iter()
        .flatten()
        .find(|axis| axis.tag == coord.axis)
        .map_or(true, css_compatible))
      .collect();

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
      coords.push(Coordinate{ axis:axis_tag(tag), value:*value });
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

// check whether the axes use standard css value ranges (rather than an arbitrary scale like some AAT fonts do)
fn css_compatible(axis:&VariationAxis) -> bool{
  match axis.tag{
    tag if tag == Coordinate::wght => (100.0..=1000.0).contains(&axis.def),
    tag if tag == Coordinate::wdth => axis.def >= 10.0,
    _ => true,
  }
}

fn axis_tag(tag:&str) -> FourByteTag{
  let b = tag.as_bytes();
  FourByteTag::from_chars(b[0] as char, b[1] as char, b[2] as char, b[3] as char)
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
