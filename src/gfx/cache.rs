// Caches for gpu-backed (and gpu-adjacent) page resources. This module owns the storage,
// render-thread lifetime, and eviction policy for the backends' cached page rasters:
//
//   • SurfaceCache — the render thread's live RecordingSurfaces, keyed by PageRecorder id. The
//     RecordingSurface values are defined over in gfx::page (where they interoperate with the
//     RasterCache); this module owns only their storage and render-thread lifetime.
//
//   • RasterCache — persistent snapshot bitmaps (Raster) of the last raster generated for a
//     given page, keyed by page id. Read/written from both the render thread and CPU exports.
//
//   • Frame — the last on-screen raster a windowed renderer painted, held as a single slot per
//     renderer (not a keyed lookup) and reused as the backdrop the next frame composites onto.
//
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use skia_safe::{Color4f, ColorSpace, Image as SkImage};
use dashmap::DashMap;

use crate::gfx::page::{ExportOptions, RecordingSurface};
use crate::mem;

#[cfg(feature = "window")]
use skia_safe::{Color, Rect, Matrix};
#[cfg(feature = "window")]
use crate::gfx::page::Page;

// release every gpu-derived cache resource: called by the render thread just before it releases an
// idle gpu context, since both live surfaces and texture-backed snapshots are bound to that context
pub fn evict_idle(){
  SurfaceCache::evict();
  RasterCache::evict_textures();
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
// RasterCache: the last bitmap generated for a given page, keyed by page id
//

static CACHE: OnceLock<Arc<DashMap<usize, Raster>>> = OnceLock::new();

pub struct RasterCache;

impl RasterCache{
  fn shared<'a>() -> &'a Arc<DashMap<usize, Raster>>{
    CACHE.get_or_init(|| Arc::new(DashMap::new()))
  }

  pub fn add(id:usize){
    Self::shared().insert(id, Raster::default());
  }

  // remove the entry but also return it (in case it contains textures that need to be dropped on the render thread)
  pub fn drop(id:usize) -> Option<Raster>{
    Self::shared().remove(&id).map(|(_, cache)| cache)
  }

  pub fn get(id:usize, opts:&ExportOptions, depth:usize, gpu:bool) -> (Option<SkImage>, usize){
    Self::shared().get(&id).map(|raster|{
      // only give access to texture-backed entries is called from the gpu context's thread
      let compatible = gpu || !raster.is_texture_backed();
      match compatible && raster.is_valid(opts) && depth >= raster.depth{
        true => (raster.image.clone(), raster.depth),
        false => (None, 0)
      }
    })
    .unwrap_or((None, 0))
  }

  pub fn set(id:usize, image:SkImage, opts:&ExportOptions, depth:usize){
    Self::shared().get_mut(&id).map(|mut raster|{
      // save the bitmap if it's newer than the cached version, or is replacing an invaildated cache
      if !raster.is_valid(opts) || depth > raster.depth{
        *raster = Raster::new(image, opts, depth);
      }
    });
  }

  // drop texture-backed cache entries: called by the render thread just before it releases an idle
  // gpu context, since the textures those entries hold are derived from that context
  pub fn evict_textures(){
    Self::shared().iter_mut().for_each(|mut raster|{
      if raster.is_texture_backed(){
        raster.footprint.clear(); // report the snapshot's dealloc to v8
        raster.image = None;
        raster.depth = 0;
      }
    });
  }
}

//
// Raster: a cached Page snapshot and the export settings that were used to render it
//

#[derive(Debug)]
pub(crate) struct Raster{
  image: Option<SkImage>,
  footprint: mem::v8::Footprint,
  density: f32,
  matte: Option<Color4f>,
  msaa: Option<usize>,
  color_space: ColorSpace,
  depth: usize,
}

impl Default for Raster{
  fn default() -> Self {
    Self{image:None, footprint:mem::v8::Footprint::default(), depth:0, density:1.0, matte:None, msaa:None, color_space:ColorSpace::new_srgb()}
  }
}

impl Raster{
  // build a cached raster from a freshly-rendered snapshot and the opts it was rendered under
  fn new(image:SkImage, opts:&ExportOptions, depth:usize) -> Self{
    let footprint = mem::v8::Footprint::new(image.image_info().compute_min_byte_size()); // report the image/texture allocation size to v8
    Self{ image:Some(image), footprint, density:opts.density, matte:opts.matte, msaa:opts.msaa, color_space:opts.color_space.clone(), depth }
  }

  // whether the cached snapshot holds a gpu texture (so its drop must run on the render thread)
  pub(crate) fn is_texture_backed(&self) -> bool{
    self.image.as_ref().map(|img| img.is_texture_backed()).unwrap_or(false)
  }

  pub fn is_valid(&self, opts:&ExportOptions) -> bool{
    self.density == opts.density &&
    self.matte == opts.matte &&
    self.msaa == opts.msaa &&
    self.color_space == opts.color_space &&
    self.image.is_some() &&
    opts.is_raster()
  }
}

//
// Frame: bitmap of the last on-screen render for a particular window (to be used as a backdrop for additive draw ops)
//

#[cfg(feature = "window")]
pub struct Frame {
    image: Option<SkImage>,
    content: Rect,
    page: Page,
    matte: Color,
    dpr: f32,
    state: FrameState,
}

#[cfg(feature = "window")]
impl Default for Frame{
    fn default() -> Self {
        Self{image:None, content:Rect::new_empty(), page:Page::default(), dpr:0.0, matte:Color::TRANSPARENT, state:FrameState::Clean}
    }
}

#[cfg(feature = "window")]
impl Frame{
    pub fn validate(&mut self, page:&Page, matte:Color, dpr:f32, clip:Rect) -> Option<(&SkImage, &Rect, Rect)>{
        if
            self.state == FrameState::Dirty ||
            self.page.id != page.id ||
            self.matte != matte ||
            self.dpr != dpr
        {
            *self = Self::default();
        }

        self.image.as_ref().map(|img| {
            let (dst, _) = Matrix::scale((dpr, dpr)).map_rect(clip);
            (img, &self.content, dst)
        })
    }

    pub fn depth(&self) -> usize {
        self.page.layers.len()
    }

    pub fn wants_snapshot(&self, page:&Page, matte:Color, dpr:f32, last_id:usize) -> bool{
        // decide (before rendering) whether the frame's snapshot would ever be drawn: skip the
        // GPU→GPU copy when it would just be discarded
        if self.state == FrameState::Resizing{
            return false // update() drops the image during resizes
        }

        let cache_is_current = self.state == FrameState::Clean &&
            self.image.is_some() &&
            self.page.id == page.id &&
            self.matte == matte &&
            self.dpr == dpr;

        match cache_is_current{
            // only re-snapshot when new layers extend the cached content
            true => page.depth() > self.page.depth(),
            // otherwise snapshot only for pages that persist across frames (an id that churns
            // frame-to-frame means full-page redraws that invalidate the cache before reuse)
            false => page.id == last_id
        }
    }

    pub fn update(&mut self, image:Option<SkImage>, page:&Page, matte:Color, dpr:f32, content:Rect){
        if self.state==FrameState::Resizing{
            // mark the framebuffer as needing a full redraw and skip updating cached image during resize
            self.state = FrameState::Dirty;
        }else if let Some(image) = image{
            // replace snapshot if new drawing has been layered on top
            let state = FrameState::Clean;
            let (content, _) = Matrix::scale((dpr, dpr)).map_rect(content);
            *self = Self{image: Some(image), page:page.clone(), matte, dpr, content, state};
        }
    }

    // a resizing invalidates the cached frame until the next full redraw completes
    pub fn start_resizing(&mut self){
        self.state = FrameState::Resizing;
    }

    // only metal needs this (used so it can draw synchronously during the resize)
    #[cfg_attr(not(feature = "metal"), allow(dead_code))]
    pub fn is_resizing(&self) -> bool{
        self.state == FrameState::Resizing
    }
}

#[cfg(feature = "window")]
#[derive(Debug, PartialEq, Clone, Copy)]
enum FrameState{
    Clean,
    Dirty,
    Resizing
}
