//
// Font collection management
//
#![allow(non_snake_case)]
use std::sync::{OnceLock};
use std::cell::RefCell;
use std::fs;
use std::path::Path;
use std::collections::HashMap;
use neon::prelude::*;

use skia_safe::{FontMgr, FontArguments, Typeface};
use skia_safe::font_style::{FontStyle, Slant};
use skia_safe::font_arguments::{VariationPosition, variation_position::Coordinate};
use skia_safe::textlayout::{FontCollection, TypefaceFontProvider, TextStyle};
use skia_safe::utils::OrderedFontMgr;

use crate::utils::*;
use crate::typography::{FontSpec, from_width, from_slant, typeface_wght_range, typeface_details};

#[cfg(target_os = "windows")]
use allsorts::{
  binary::read::ReadScope,
  subset::whole_font,
  tables::FontTableProvider,
  woff::WoffFont,
  woff2::Woff2Font,
};

thread_local!( static LIBRARY: OnceLock<RefCell<FontLibrary>> = const{ OnceLock::new() }; );

#[derive(PartialEq, Eq, Hash)]
pub struct CollectionKey{ families:String, weight:i32, slant:Slant }

impl CollectionKey{
  pub fn new(style: &TextStyle) -> Self {
    let families = style.font_families();
    let families = families.iter().collect::<Vec<&str>>().join(", ");
    let weight = *style.font_style().weight();
    let slant = style.font_style().slant();
    CollectionKey{ families, weight, slant }
  }
}

pub struct FontLibrary{
    pub mgr: FontMgr,
    pub fonts: Vec<(Typeface, Option<String>)>,
    pub generics: Vec<(Typeface, Option<String>)>,
    pub collection: Option<FontCollection>,
    collection_cache: HashMap<CollectionKey, FontCollection>,
    collection_hinted: bool,
  }

// TODO: fix indentation...
  impl FontLibrary{
    pub fn with_shared<T, F>(f:F) -> T
      where F:FnOnce(&mut FontLibrary) -> T
    {
      LIBRARY.with(|lib_lock|{
        let shared_lib = lib_lock.get_or_init(||{
          RefCell::new(FontLibrary{
            mgr:FontMgr::default(), collection:None, collection_cache:HashMap::new(), collection_hinted:false, fonts:vec![], generics:vec![]
          })
        });

        f(&mut shared_lib.borrow_mut())
      })
    }

    fn font_collection(&mut self) -> FontCollection{
      // lazily initialize font collection on first actual use
      if self.collection.is_none(){
        // set up generic font family mappings
        if self.generics.is_empty(){
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
          self.generics = generics;
        }

        self.rebuild_collection(); // assigns to self.collection
      };

      self.collection.as_ref().unwrap().clone()
    }

    pub fn font_mgr(&mut self) -> FontMgr {
      // collect non-system fonts in a provider
      let mut dyn_mgr = TypefaceFontProvider::new();

      // add a sensible fallback as the first font so the default isn't just whatever is alphabetically first
      if let Some(fallback) = self.font_collection()
        .find_typefaces(&["system-ui", "sans-serif", "serif"], FontStyle::normal())
        .into_iter().nth(0){ dyn_mgr.register_typeface(fallback, None); }

      // add generic mappings & user-loaded fonts
      for (font, alias) in &self.generics{
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

    fn family_details(&self, family:&str) -> (Vec<f32>, Vec<String>, Vec<String>){
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
      all_styles.for_each(|(style, _name)| {
        widths.push(from_width(style.width()));
        styles.push(from_slant(style.slant()));
        weights.push(*style.weight());
        if let Some(font) = var_fc.find_typefaces(&[&family], style).first(){
          // for variable fonts, report all the 100× sizes they support within their wght range
          weights.append(&mut typeface_wght_range(font));
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
      // make sure the collection & generics have been initialized before starting
      self.font_collection();

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
      self.rebuild_collection();
    }

    fn rebuild_collection(&mut self){
      let mut assets = TypefaceFontProvider::new();
      for (font, alias) in &self.generics {
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
      self.collection = Some(collection);
      self.collection_cache.drain();
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
          style.reset_font_features();
          for (feat, val) in &spec.features{
            style.add_font_feature(feat, *val);
          }
          style
        })
    }

    pub fn set_hinting(&mut self, hinting:bool) -> &mut Self{
      // skia's rasterizer cache doesn't take hinting into account, so manually invalidate if changed
      if hinting != self.collection_hinted{
        self.collection_hinted = hinting;
        self.collection_cache.iter_mut().for_each(|(_, fc)| fc.clear_caches());
        self.font_collection().clear_caches();
      }
      self
    }

    pub fn collect_fonts(&mut self, style: &TextStyle) -> FontCollection {
      let families = style.font_families();
      let families:Vec<&str> = families.iter().collect();
      let matches = self.font_collection().find_typefaces(&families, style.font_style());

      // if the matched typeface is a variable font, create an instance that matches
      // the current weight settings and return early with a new FontCollection that
      // contains just that single font instance
      if let Some(font) = matches.first() {
        if let Some(params) = font.variation_design_parameters(){

          // memoize the generation of single-weight FontCollections for variable fonts
          let key = CollectionKey::new(style);
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
              dynamic.register_typeface(face, alias.as_deref());

              let mut collection = FontCollection::new();
              collection.set_default_font_manager(self.mgr.clone(), None);
              collection.set_asset_font_manager(Some(dynamic.into()));
              self.collection_cache.insert(key, collection.clone());
              return collection
            }
          }
        }
      }

      // if the matched font wasn't variable, then just return the standard collection
      self.font_collection()
    }

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
    let (weights, widths, styles) = FontLibrary::with_shared(|lib|
      lib.family_details(&family)
    );

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
    let results = JsArray::new(&mut cx, filenames.len());

    for (i, filename) in strings_in(&mut cx, &filenames).iter().enumerate(){
      let path = Path::new(&filename);
      let typeface = match fs::read(path){
        Err(why) => {
          return cx.throw_error(format!("{}: \"{}\"", why, path.display()))
        },
        Ok(bytes) => {
          #[cfg(target_os = "windows")]
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
      lib.collection = None;
      lib.collection_cache.drain();
    });

    Ok(cx.undefined())
  }
