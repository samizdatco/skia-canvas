// Caches for gpu-backed (and gpu-adjacent) page resources. This module owns the storage,
// render-thread lifetime, and eviction policy for two distinct caches:
//
//   • SurfaceCache — the render thread's live RecordingSurfaces, keyed by PageRecorder id. The
//     RecordingSurface values are defined over in context::page (where they interoperate with the
//     FrameCache); this module owns only their storage and render-thread lifetime.
//
//   • FrameCache — persistent snapshot bitmaps (Frame) of the last raster generated for a
//     given page, keyed by page id. Read/written from both the render thread and CPU exports.
//
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use skia_safe::{Color, Image as SkImage};
use dashmap::DashMap;

use crate::context::page::{ExportOptions, RecordingSurface};

// release every gpu-derived cache resource: called by the render thread just before it releases an
// idle gpu context, since both live surfaces and texture-backed snapshots are bound to that context
pub fn evict_idle(){
  SurfaceCache::evict();
  FrameCache::evict_textures();
}

//
// SurfaceCache: the render thread's live getImageData surfaces, keyed by PageRecorder id
//

thread_local!(
    // gpu-backed RecordingSurfaces, keyed by PageRecorder id: only ever touched on the render
    // thread (via engine.render/render_soon jobs), so gpu surfaces never cross threads
    static RECORDING_SURFACES: RefCell<HashMap<usize, RecordingSurface>> = RefCell::new(HashMap::new());
);

pub struct SurfaceCache;

impl SurfaceCache{
  // run `f` against the surface for `id`, creating a default entry if none exists yet
  pub fn with_entry<T>(id:usize, f:impl FnOnce(&mut RecordingSurface) -> T) -> T {
    RECORDING_SURFACES.with_borrow_mut(|surfaces| f(surfaces.entry(id).or_default()))
  }

  // run `f` against the surface for `id` only if one already exists (no insert)
  pub fn with_existing<T>(id:usize, f:impl FnOnce(&mut RecordingSurface) -> T) -> Option<T> {
    RECORDING_SURFACES.with_borrow_mut(|surfaces| surfaces.get_mut(&id).map(f))
  }

  // drop the surface for a single recorder (its PageRecorder is being released/dropped)
  pub fn drop(id:usize){
    RECORDING_SURFACES.with_borrow_mut(|surfaces|{ surfaces.remove(&id); })
  }

  // drop every recording surface: called by the render thread just before it releases an idle gpu
  // context, since everything derived from that context must be released along with it
  pub fn evict(){
    RECORDING_SURFACES.with_borrow_mut(|surfaces| surfaces.clear());
  }
}

//
// FrameCache: the last bitmap generated for a given page, keyed by page id
//

static CACHE: OnceLock<Arc<DashMap<usize, Frame>>> = OnceLock::new();

pub struct FrameCache;

impl FrameCache{
  fn shared<'a>() -> &'a Arc<DashMap<usize, Frame>>{
    CACHE.get_or_init(|| Arc::new(DashMap::new()))
  }

  pub fn add(id:usize){
    Self::shared().insert(id, Frame::default());
  }

  // remove the entry but also return it (in case it contains textures that need to be dropped on the render thread)
  pub fn drop(id:usize) -> Option<Frame>{
    Self::shared().remove(&id).map(|(_, cache)| cache)
  }

  pub fn get(id:usize, opts:&ExportOptions, depth:usize, gpu:bool) -> (Option<SkImage>, usize){
    Self::shared().get(&id).map(|frame|{
      // only give access to texture-backed entries is called from the gpu context's thread
      let compatible = gpu || !frame.is_texture_backed();
      match compatible && frame.is_valid(opts) && depth >= frame.depth{
        true => (frame.image.clone(), frame.depth),
        false => (None, 0)
      }
    })
    .unwrap_or((None, 0))
  }

  pub fn set(id:usize, image:SkImage, opts:&ExportOptions, depth:usize){
    Self::shared().get_mut(&id).map(|mut frame|{
      // save the bitmap if it's newer than the cached version, or is replacing an invaildated cache
      if !frame.is_valid(opts) || depth > frame.depth{
        *frame = Frame::new(image, opts, depth);
      }
    });
  }

  // drop texture-backed cache entries: called by the render thread just before it releases an idle
  // gpu context, since the textures those entries hold are derived from that context
  pub fn evict_textures(){
    Self::shared().iter_mut().for_each(|mut frame|{
      if frame.is_texture_backed(){
        frame.image = None;
        frame.depth = 0;
      }
    });
  }
}

//
// Frame: a cached Page snapshot and the export settings that were used to render it
//

#[derive(Debug, Clone)]
pub(crate) struct Frame{
  image: Option<SkImage>,
  density: f32,
  matte: Option<Color>,
  msaa: Option<usize>,
  depth: usize,
}

impl Default for Frame{
  fn default() -> Self {
    Self{image:None, depth:0, density:1.0, matte:None, msaa:None}
  }
}

impl Frame{
  // build a cached frame from a freshly-rendered snapshot and the opts it was rendered under
  fn new(image:SkImage, opts:&ExportOptions, depth:usize) -> Self{
    Self{ image:Some(image), density:opts.density, matte:opts.matte, msaa:opts.msaa, depth }
  }

  // whether the cached snapshot holds a gpu texture (so its drop must run on the render thread)
  pub(crate) fn is_texture_backed(&self) -> bool{
    self.image.as_ref().map(|img| img.is_texture_backed()).unwrap_or(false)
  }

  pub fn is_valid(&self, opts:&ExportOptions) -> bool{
    self.density == opts.density &&
    self.matte == opts.matte &&
    self.msaa == opts.msaa &&
    self.image.is_some() &&
    opts.is_raster()
  }
}
