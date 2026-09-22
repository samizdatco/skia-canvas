//
// Font collection management
//
#![allow(non_snake_case)]
use std::sync::{OnceLock};
use std::borrow::Cow;
use std::cell::RefCell;
use std::fs;
use std::path::Path;
use std::collections::HashMap;
use neon::prelude::*;

use skia_safe::{font::Edging, FontHinting, FontMgr, Typeface};
use skia_safe::font_style::FontStyle;
use skia_safe::textlayout::{FontCollection, TypefaceFontProvider, TextStyle};
use skia_safe::utils::OrderedFontMgr;

use crate::bridge::*;
use kurbo::{BezPath, CubicBez, PathEl, Point};
use read_fonts::{model::pen::OutlinePen, types::GlyphId, ps::{cff::CffFontRef, type1::Type1Font}};
use write_fonts::{
  FontBuilder, types::{FWord, NameId, UfWord},
  tables::{
    cmap::Cmap, glyf::{Glyph, GlyfLocaBuilder, SimpleGlyph}, head::{Flags, Head}, hhea::Hhea, hmtx::{Hmtx, LongMetric},
    maxp::Maxp, name::{Name, NameRecord}, os2::{Os2, SelectionFlags}, post::Post,
  },
};
use allsorts::{
  binary::read::ReadScope,
  subset::whole_font,
  tables::FontTableProvider,
  woff::WoffFont,
  woff2::Woff2Font,
};

thread_local!( static LIBRARY: OnceLock<RefCell<FontLibrary>> = const{ OnceLock::new() }; );

pub struct FontLibrary{
    mgr: FontMgr,
    collection: Option<FontCollection>,
    fonts: Vec<(Typeface, Option<String>)>,
    generics_cache: Vec<(Typeface, Option<String>)>,
    collection_attrs: RenderAttrs,
  }

impl FontLibrary{
  pub fn with_shared<T, F>(f:F) -> T
    where F:FnOnce(&mut FontLibrary) -> T
  {
    LIBRARY.with(|lib_lock|{
      let shared_lib = lib_lock.get_or_init(||{
        // detect linux systems without a working fontconfig setup and use the fallback config in `lib/fonts` instead
        #[cfg(target_os = "linux")]
        {
          let has_config = fs::exists(Path::new("/etc/fonts/fonts.conf")).unwrap_or(false);
          let has_override = std::env::var_os("FONTCONFIG_PATH").is_some() || std::env::var_os("FONTCONFIG_FILE").is_some();
          if !(has_config || has_override){
            if let Some(mut fallback_config_path) = process_path::get_dylib_path(){
              fallback_config_path.set_file_name("fonts");
              unsafe { std::env::set_var("FONTCONFIG_PATH", fallback_config_path); }
            }
          }
        }

        RefCell::new(FontLibrary{
          mgr:FontMgr::default(), fonts:vec![], collection:None, collection_attrs:RenderAttrs::default(), generics_cache:vec![]
        })
      });

      f(&mut shared_lib.borrow_mut())
    })
  }

  pub fn is_empty() -> bool{
    Self::with_shared(|lib| lib.families().is_empty())
  }

  pub fn font_collection(&mut self) -> FontCollection{
    // lazily initialize font collection on first actual use
    if self.collection.is_none(){
      self.collection = Some(self.new_font_collection());
    };

    self.collection.as_ref().unwrap().clone()
  }

  fn new_font_collection(&mut self) -> FontCollection{
    let mut assets = TypefaceFontProvider::new();
    for (font, alias) in self.generics() {
      assets.register_typeface(font.clone(), alias.as_deref());
    }
    for (font, alias) in &self.fonts {
      assets.register_typeface(font.clone(), alias.as_deref());
    }

    let mut style_set = assets.match_family("system-ui");
    let default_fam = match style_set.count() > 1{
      true => style_set.match_style(FontStyle::default()),
      false => self.mgr.legacy_make_typeface(None, FontStyle::default())
    }.map(|f| f.family_name());

    let mut collection = FontCollection::new();
    collection.set_default_font_manager(self.mgr.clone(), default_fam.as_deref());
    collection.set_asset_font_manager(Some(assets.into()));
    collection
  }

  fn generics(&mut self) -> &Vec<(Typeface, Option<String>)> {
    // set up generic font family mappings
    if self.generics_cache.is_empty(){
      let mut generics = vec![];
      let mut font_stacks = HashMap::new();
      font_stacks.insert("serif", vec!["Times", "Nimbus Roman", "Times New Roman", "Tinos", "Noto Serif", "Liberation Serif", "DejaVu Serif", "Source Serif Pro"]);
      font_stacks.insert("sans-serif", vec!["Avenir Next", "Avenir", "Helvetica Neue", "Helvetica", "Arial Nova", "Arial", "Inter", "Arimo", "Roboto", "Noto Sans", "Liberation Sans", "DejaVu Sans", "Nimbus Sans", "Clear Sans", "Lato", "Cantarell", "Arimo", "Ubuntu"]);
      font_stacks.insert("monospace", vec!["Cascadia Code", "Source Code Pro", "Menlo", "Consolas", "Monaco", "Liberation Mono", "Ubuntu Mono", "Roboto Mono", "Lucida Console", "Monaco", "Courier New", "Courier"]);
      font_stacks.insert("system-ui", vec!["Helvetica Neue", "Ubuntu", "Segoe UI", "Fira Sans", "Roboto", "DroidSans", "Tahoma"]);
      // see also: https://modernfontstacks.com | https://systemfontstack.com | https://www.ctrl.blog/entry/font-stack-text.html

      // Set up mappings for generic font names based on the first match found on the system
      for (generic_name, families) in font_stacks.into_iter() {
        let best_match = families.iter().find_map(|fam| {
          let mut style_set = self.mgr.match_family(fam);
          match style_set.count() > 0{
            true => Some(style_set),
            false => None
          }
        });

        let alias = Some(generic_name.to_string());
        if let Some(mut style_set) = best_match{
          for style_index in 0..style_set.count() {
            if let Some(font) = style_set.new_typeface(style_index){
              generics.push((font, alias.clone()));
            }
          }
        }
      }
      self.generics_cache = generics;
    }

    &self.generics_cache
  }

  pub fn font_mgr(&mut self) -> FontMgr {
    // collect non-system fonts in a provider
    let mut dyn_mgr = TypefaceFontProvider::new();

    // add a sensible fallback as the first font so the default isn't just whatever is alphabetically first
    if let Some(fallback) = self.font_collection()
      .find_typefaces(&["system-ui", "sans-serif", "serif"], FontStyle::normal())
      .into_iter().nth(0){ dyn_mgr.register_typeface(fallback, None); }

    // add generic mappings & user-loaded fonts
    for (font, alias) in self.generics(){
      dyn_mgr.register_typeface(font.clone(), alias.as_deref());
    }
    for (font, alias) in &self.fonts{
      dyn_mgr.register_typeface(font.clone(), alias.as_deref());
    }

    // merge system & non-system fonts into single FontMgr
    let mut union_mgr = OrderedFontMgr::new();
    union_mgr.append(dyn_mgr); // generics & user-loaded fonts
    union_mgr.append(self.mgr.clone()); // system fonts
    union_mgr.into()
  }

  fn families(&self) -> Vec<String>{
    let mut names:Vec<String> = self.mgr.family_names().collect();
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

  fn family_details(&self, family:&str) -> (Vec<f32>, Vec<String>, Vec<String>, FontCapabilities){
    // merge the system fonts and our dynamically added fonts into one list of FontStyles
    let mut dynamic = TypefaceFontProvider::new();
    for (font, alias) in &self.fonts{
      dynamic.register_typeface(font.clone(), alias.as_deref());
    }
    let std_mgr = self.mgr.clone();
    let dyn_mgr:FontMgr = dynamic.into();
    let mut std_set = std_mgr.match_family(family);
    let mut dyn_set = dyn_mgr.match_family(family);
    let std_styles = (0..std_set.count()).map(|i| std_set.style(i));
    let dyn_styles = (0..dyn_set.count()).map(|i| dyn_set.style(i));
    let all_styles = std_styles.chain(dyn_styles);

    // set up a collection to query for variable fonts who specify their weights
    // via the 'wght' axis rather than through distinct files with different FontStyles
    let mut var_fc = FontCollection::new();
    var_fc.set_default_font_manager(self.mgr.clone(), None);
    var_fc.set_asset_font_manager(Some(dyn_mgr));

    // pull style values out of each matching font
    let mut weights:Vec<i32> = vec![];
    let mut widths:Vec<String> = vec![];
    let mut styles:Vec<String> = vec![];
    let mut caps = FontCapabilities::default();
    all_styles.for_each(|(style, _name)| {
      widths.push(from_width(style.width()));
      styles.push(from_slant(style.slant()));
      weights.push(*style.weight());
      if let Some(font) = var_fc.find_typefaces(&[&family], style).first(){
        let face = FontCapabilities::new(font);
        weights.append(&mut face.wght_range()); // if variable: include all supported 100x sizes
        caps.merge(face); // merge variable font axes and opentype features
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
    (weights, widths, styles, caps)
  }

  fn add_typeface(&mut self, font:Typeface, alias:Option<String>){
    // replace any previously added font with the same metadata/alias
    if let Some(idx) = self.fonts.iter().position(|(old_font, old_alias)|
      match alias.is_some(){
        true => old_alias == &alias,
        false => old_font.family_name() == font.family_name()
      } && old_font.font_style() == font.font_style()
    ){
      self.fonts.remove(idx);
    }

    // add the new typeface/alias and recreate the FontCollection to include it
    self.fonts.push((font, alias));
    self.invalidate();
  }

  fn invalidate(&mut self){
    // the font collection and metrics cache always need to be invalidated in tandem
    self.collection = None;
    METRICS_CACHE.with_borrow_mut(|cache| *cache = MetricsCache::default());
  }

  pub fn update_style(&mut self, orig_style:&TextStyle, spec: &FontSpec) -> Option<TextStyle>{
    let mut style = orig_style.clone();

    // only update the style if a usable family name was specified
    self.font_collection()
      .find_typefaces(&spec.families, spec.style())
      .into_iter().nth(0)
      .map(|typeface| {
        style.set_typeface(typeface);
        style.set_font_families(&spec.families);
        style.set_font_style(spec.style());
        style.set_font_size(spec.size);
        style
      })
  }

  pub fn set_render_attrs(&mut self, attrs:RenderAttrs) -> &mut Self{
    // skia's layout & font-resolution caches don't key on any of these attrs, so manually invalidate if changed
    if attrs != self.collection_attrs{
      self.collection_attrs = attrs;
      self.font_collection().clear_caches();
    }
    self
  }

  // return data that's already an sfnt as-is and convert Type1/CFF data to TTF
  pub fn sfnt_from(data:&[u8]) -> Option<Cow<'_, [u8]>>{
    if matches!(data.get(..4), Some(b"OTTO" | b"true" | b"ttcf" | [0, 1, 0, 0])){
      Some(Cow::Borrowed(data)) // already an sfnt
    }else if data.first() == Some(&1) && data.get(2).is_some_and(|hdr_size| *hdr_size >= 4){
      Glyphs::from_cff(data)?.to_ttf().map(Cow::Owned)
    }else if data.starts_with(b"%!") || data.starts_with(&[0x80, 1]){
      Glyphs::from_type1(data)?.to_ttf().map(Cow::Owned)
    }else{
      None
    }
  }
}

//
// TrueType synthesis from the outlines of Type 1 & CFF fonts (which would otherwise be drawn as paths)
//

// the glyphs of a Type 1 or CFF font, drawn one at a time into kurbo paths
#[derive(Default)]
struct Glyphs{
  ps_name: String,                       // the original font's FontName
  outlines: Vec<(BezPath, Option<f32>)>, // each glyph's path & advance in charstring order
  outline: BezPath,                      // the glyph currently being drawn (see OutlinePen impl)
}

const UNITS_PER_EM:f32 = 1000.0; // hayro normalises its outlines to this em as well
const ACCURACY:f64 = 1.0;        // max deviation of the quadratic approximation, in font units

impl Glyphs{
  // a bare CFF font (from a PDF's FontFile3 stream)
  fn from_cff(data:&[u8]) -> Option<Self>{
    let font = CffFontRef::new(data, 0, None).ok()?;
    let subfonts = (0..font.num_subfonts()).map(|i| font.subfont(i, &[]).ok()).collect::<Option<Vec<_>>>()?;
    let mut glyphs = Glyphs{ ps_name:font.metadata().and_then(|meta| meta.name()).unwrap_or("CFF").to_string(), ..Default::default() };
    for gid in (0..font.num_glyphs()).map(GlyphId::new){
      let advance = font.subfont_index(gid).and_then(|i| subfonts.get(i as usize))
        .and_then(|subfont| font.draw(subfont, gid, &[], Some(UNITS_PER_EM), &mut glyphs).ok().flatten());
      glyphs.outlines.push((std::mem::take(&mut glyphs.outline), advance));
    }
    Some(glyphs)
  }

  // a pfa or pfb Type 1 font (from a PDF's FontFile stream)
  fn from_type1(data:&[u8]) -> Option<Self>{
    let font = Type1Font::new(data).ok()?;
    let mut glyphs = Glyphs{ ps_name:font.name().unwrap_or("Type1").to_string(), ..Default::default() };
    for gid in (0..font.num_glyphs()).map(GlyphId::new){
      let advance = font.draw(gid, Some(UNITS_PER_EM), &mut glyphs).ok().flatten();
      glyphs.outlines.push((std::mem::take(&mut glyphs.outline), advance));
    }
    Some(glyphs)
  }

  fn to_ttf(self) -> Option<Vec<u8>>{
    let ps_name = self.ps_name.as_str();
    let mut glyf = GlyfLocaBuilder::new();
    let (mut metrics, mut bbox, mut max_points, mut max_contours) = (vec![], [0i16; 4], 0, 0);
    for (path, advance) in self.outlines{
      // replace each cubic with quadratics
      let mut last = Point::ZERO;
      let quadratic:BezPath = path.elements().iter().flat_map(|el| match *el{
        PathEl::CurveTo(c1, c2, p) => {
          let quads = CubicBez::new(last, c1, c2, p).to_quads(ACCURACY).map(|(_, _, q)| PathEl::QuadTo(q.p1, q.p2)).collect::<Vec<_>>();
          last = p;
          quads
        }
        el => { if let Some(p) = el.end_point(){ last = p } vec![el] }
      }).collect();

      let glyph = match SimpleGlyph::from_bezpath(&quadratic){
        Ok(g) if !g.contours.is_empty() => g,
        _ => { glyf.add_glyph(&Glyph::Empty).ok()?; metrics.push(LongMetric{ advance:advance.unwrap_or(0.0) as u16, side_bearing:0 }); continue }
      };
      let b = glyph.bbox;
      bbox = [bbox[0].min(b.x_min), bbox[1].min(b.y_min), bbox[2].max(b.x_max), bbox[3].max(b.y_max)];
      max_points = max_points.max(glyph.contours.iter().map(|c| c.len()).sum::<usize>());
      max_contours = max_contours.max(glyph.contours.len());
      // TrueType rasterizers shift each outline so that xMin lands on its left sidebearing
      metrics.push(LongMetric{ advance:advance.unwrap_or(0.0).round().clamp(0.0, 65535.0) as u16, side_bearing:b.x_min });
      glyf.add_glyph(&glyph).ok()?;
    }
    let (glyf, loca, loca_format) = glyf.build();
    let [x_min, y_min, x_max, y_max] = bbox;
    let n = metrics.len() as u16;

    let head = Head{
      flags:Flags::BASELINE_AT_Y_0 | Flags::LSB_AT_X_0 | Flags::FORCE_INTEGER_PPEM, units_per_em:UNITS_PER_EM as u16, x_min, y_min, x_max, y_max,
      lowest_rec_ppem:8, font_direction_hint:2, index_to_loc_format:loca_format as i16, ..Default::default()
    };
    let hhea = Hhea::new(
      FWord::new(y_max), FWord::new(y_min), FWord::new(0), UfWord::new(metrics.iter().map(|m| m.advance).max().unwrap_or(0)),
      FWord::new(x_min), FWord::new(0), FWord::new(x_max), 1, 0, 0, n,
    );
    let maxp = Maxp{
      max_points:Some(max_points as u16), max_contours:Some(max_contours as u16), max_composite_points:Some(0), max_composite_contours:Some(0), max_zones:Some(1),
      max_twilight_points:Some(0), max_storage:Some(0), max_function_defs:Some(0), max_instruction_defs:Some(0), max_stack_elements:Some(0),
      max_size_of_instructions:Some(0), max_component_elements:Some(0), max_component_depth:Some(0), ..Maxp::new(n)
    };
    let os2 = Os2{
      x_avg_char_width:(metrics.iter().map(|m| m.advance as u32).sum::<u32>() / n.max(1) as u32) as i16, us_weight_class:400, us_width_class:5,
      fs_selection:SelectionFlags::REGULAR, us_first_char_index:0xFFFF, us_last_char_index:0xFFFF, s_typo_ascender:y_max, s_typo_descender:y_min,
      us_win_ascent:y_max.max(0) as u16, us_win_descent:(-y_min).max(0) as u16, ul_code_page_range_1:Some(0), ul_code_page_range_2:Some(0),
      sx_height:Some(0), s_cap_height:Some(0), us_default_char:Some(0), us_break_char:Some(32), us_max_context:Some(0), ..Default::default()
    };
    let postscript_name:String = ps_name.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect(); // the name table's version can't hold spaces or punctuation
    let names = [(NameId::FAMILY_NAME, ps_name), (NameId::SUBFAMILY_NAME, "Regular"), (NameId::FULL_NAME, ps_name), (NameId::POSTSCRIPT_NAME, &postscript_name)];
    let name = Name::new([(1, 0, 0), (3, 1, 0x409)].into_iter() // mac roman, then windows unicode (records must stay sorted)
      .flat_map(|(platform, encoding, language)| names.iter().map(move |(id, s)| NameRecord::new(platform, encoding, language, *id, s.to_string().into()))).collect());

    let mut font = FontBuilder::new();
    font.add_table(&head).ok()?.add_table(&hhea).ok()?.add_table(&Hmtx::new(metrics, vec![])).ok()?.add_table(&maxp).ok()?
      .add_table(&glyf).ok()?.add_table(&loca).ok()?.add_table(&os2).ok()?.add_table(&Post::default()).ok()?.add_table(&name).ok()?
      .add_table(&Cmap::from_mappings([]).ok()?).ok()?; // an empty cmap: glyphs are addressed by id, unicode travels with the text runs
    Some(font.build())
  }
}

// add `read-fonts` support and drop segments that arrive before a move_to
impl OutlinePen for Glyphs{
  fn move_to(&mut self, x:f32, y:f32){ self.outline.move_to((x as f64, y as f64)) }
  fn line_to(&mut self, x:f32, y:f32){ if !self.outline.elements().is_empty(){ self.outline.line_to((x as f64, y as f64)) } }
  fn quad_to(&mut self, cx:f32, cy:f32, x:f32, y:f32){
    if !self.outline.elements().is_empty(){ self.outline.quad_to((cx as f64, cy as f64), (x as f64, y as f64)) }
  }
  fn curve_to(&mut self, cx0:f32, cy0:f32, cx1:f32, cy1:f32, x:f32, y:f32){
    if !self.outline.elements().is_empty(){ self.outline.curve_to((cx0 as f64, cy0 as f64), (cx1 as f64, cy1 as f64), (x as f64, y as f64)) }
  }
  fn close(&mut self){ if !self.outline.elements().is_empty(){ self.outline.close_path() } }
}

#[derive(Clone, Copy, PartialEq)]
pub struct RenderAttrs{
  pub hinting: FontHinting,
  pub edging: Edging,
  pub subpixel: bool,
  pub synthesize: bool,
}

impl Default for RenderAttrs{
  fn default() -> Self{
    Self{hinting:FontHinting::None, edging:Edging::AntiAlias, subpixel:true, synthesize:true}
  }
}

//
// Text metrics cache (saves repeated multi-line measurement and serialization)
//

const METRICS_CAP: usize = 2048;

pub type MetricsKey = (u64, String, Option<u32>); // (state hash, text, maxWidth bits)

#[derive(Default)]
struct MetricsCache{
  hot: HashMap<MetricsKey, String>,
  cold: HashMap<MetricsKey, String>,
}

impl MetricsCache{
  fn get(&mut self, key:&MetricsKey) -> Option<String>{
    if let Some(json) = self.hot.get(key){ return Some(json.clone()) }
    let json = self.cold.remove(key)?;
    self.put(key.clone(), json.clone()); // a cold hit is re-promoted into the current generation
    Some(json)
  }

  fn put(&mut self, key:MetricsKey, json:String){
    if self.hot.len() >= METRICS_CAP{
      self.cold = std::mem::take(&mut self.hot); // demote; the old cold generation drops here
    }
    self.hot.insert(key, json);
  }
}

thread_local!(
  static METRICS_CACHE: RefCell<MetricsCache> = RefCell::default();
);

// look up a measurement, or compute-and-store it. the borrow is *not* held across `compute`
// (house rule: never hold a store while doing real work — and here re-entry is a certainty, since
// `compute` typesets, which borrows the library)
pub fn cached_metrics(key:MetricsKey, compute:impl FnOnce() -> String) -> String{
  if let Some(json) = METRICS_CACHE.with_borrow_mut(|cache| cache.get(&key)){
    return json
  }
  let json = compute();
  METRICS_CACHE.with_borrow_mut(|cache| cache.put(key, json.clone()));
  json
}


//
// Javascript Methods
//

pub fn get_families(mut cx: FunctionContext) -> JsResult<JsArray> {
  strings_to_array(&mut cx, &FontLibrary::with_shared(|lib|
    lib.families()
  ))
}

pub fn has(mut cx: FunctionContext) -> JsResult<JsBoolean> {
  let family = string_arg(&mut cx, 1, "familyName")?;
  let found = FontLibrary::with_shared(|lib|
    lib.families().contains(&family)
  );
  Ok(cx.boolean(found))
}

pub fn family(mut cx: FunctionContext) -> JsResult<JsValue> {
  let family = string_arg(&mut cx, 1, "familyName")?;
  let (weights, widths, styles, caps) = FontLibrary::with_shared(|lib|
    lib.family_details(&family)
  );

  if weights.is_empty() {
    return Ok(cx.undefined().upcast())
  }

  let weights = floats_to_array(&mut cx, &weights)?;
  let widths = strings_to_array(&mut cx, &widths)?;
  let styles = strings_to_array(&mut cx, &styles)?;
  let variations = caps.variations_object(&mut cx)?;
  let features = caps.features_object(&mut cx)?;

  let details = JsObject::new(&mut cx);
  details.prop(&mut cx, "family").set(family)?;
  details.prop(&mut cx, "weights").set(weights)?;
  details.prop(&mut cx, "widths").set(widths)?;
  details.prop(&mut cx, "styles").set(styles)?;
  details.prop(&mut cx, "variable").set(!caps.axes.is_empty())?;
  details.prop(&mut cx, "variations").set(variations)?;
  details.prop(&mut cx, "features").set(features)?;

  Ok(details.upcast())
}

pub fn addFamily(mut cx: FunctionContext) -> JsResult<JsValue> {
  let alias = opt_string_arg(&mut cx, 1);
  let filenames = cx.argument::<JsArray>(2)?.to_vec(&mut cx)?;
  let results = JsArray::new(&mut cx, filenames.len());

  for (i, filename) in strings_in(&mut cx, &filenames).iter().enumerate(){
    let path = Path::new(&filename);
    let typeface = match fs::read(path){
      Err(why) => {
        return cx.throw_error(format!("{}: \"{}\"", why, path.display()))
      },
      Ok(bytes) => {
        // TEMPORARY WORKAROUND: decode woff/woff2 to sfnt
        //
        // originally only Windows needed this since it's the one platform that lacks built-in
        // support, but there is a use-after-free bug for woffs in harfbuzz versions <14.4.0
        // (including the one skia bundles as of m150)
        //
        // go back to using #[cfg(target_os = "windows")] when skia's harfbuzz is upgraded
        let bytes = {
          fn decode_woff(bytes:&Vec<u8>) -> Option<Vec<u8>>{
            let woff = ReadScope::new(&bytes).read::<WoffFont>().ok()?;
            let tags = woff.table_tags()?;
            whole_font(&woff, &tags).ok()
          }

          fn decode_woff2(bytes:&Vec<u8>) -> Option<Vec<u8>>{
            let woff2 = ReadScope::new(&bytes).read::<Woff2Font>().ok()?;
            let tables = woff2.table_provider(0).ok()?;
            let tags = tables.table_tags()?;
            whole_font(&tables, &tags).ok()
          }

          match filename.to_ascii_lowercase(){
            name if name.ends_with(".woff") => decode_woff(&bytes),
            name if name.ends_with(".woff2") => decode_woff2(&bytes),
            _ => None
          }
        }.unwrap_or(bytes);

        FontLibrary::with_shared(|lib|
          lib.mgr.new_from_data(&bytes, None)
        )
      }
    };

    match typeface {
      Some(font) => {
        // add family/weight/width/slant details to return value
        let details = typeface_details(&mut cx, filename, &font, alias.clone())?;
        results.set(&mut cx, i as u32, details)?;

        // register the typeface
        FontLibrary::with_shared(|lib|
          lib.add_typeface(font, alias.clone())
        );
      },
      None => {
        return cx.throw_error(format!("Could not decode font data in {}", path.display()))
      }
    }
  }

  Ok(results.upcast())
}

pub fn reset(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  FontLibrary::with_shared(|lib|{
    lib.fonts.clear();
    lib.invalidate();
  });

  Ok(cx.undefined())
}
