// Caches for gpu-backed (and gpu-adjacent) page resources. This module owns the storage,
// render-thread lifetime, and eviction policy for the backends' cached page rasters:
//
//   • SurfaceCache — the render thread's live RecordingSurfaces, keyed by PageRecorder id. The
//     RecordingSurface values are defined over in gfx::page; this module owns only their storage
//     and render-thread lifetime.
//
//   • Frame — the last on-screen raster a windowed renderer painted, held as a single slot per
//     renderer (not a keyed lookup) and reused as the backdrop the next frame composites onto.
//
use std::cell::RefCell;
use std::collections::HashMap;

use crate::gfx::page::RecordingSurface;

#[cfg(feature = "window")]
use skia_safe::{Rect, Matrix, Color4f, Image as SkImage};
#[cfg(feature = "window")]
use crate::gfx::page::Page;

// evict every live gpu surface: called by the render thread just before it retires an idle gpu
// context, since the surfaces are bound to that context
pub fn evict_idle(){
  SurfaceCache::evict_all();
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

  // evict the surface for a single recorder (its PageRecorder is being released)
  pub fn evict(id:usize){
    RECORDING_SURFACES.with_borrow_mut(|surfaces|{ surfaces.remove(&id); })
  }

  // evict every recording surface: called by the render thread just before it retires an idle gpu
  // context, since everything derived from that context must be evicted along with it
  pub fn evict_all(){
    RECORDING_SURFACES.with_borrow_mut(|surfaces| surfaces.clear());
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
    matte: Color4f,
    dpr: f32,
    state: FrameState,
}

#[cfg(feature = "window")]
impl Default for Frame{
    fn default() -> Self {
        Self{image:None, content:Rect::new_empty(), page:Page::default(), dpr:0.0, matte:Color4f::new(0.0, 0.0, 0.0, 0.0), state:FrameState::Clean}
    }
}

#[cfg(feature = "window")]
impl Frame{
    pub fn validate(&mut self, page:&Page, matte:Color4f, dpr:f32, clip:Rect) -> Option<(&SkImage, &Rect, Rect)>{
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

    pub fn wants_snapshot(&self, page:&Page, matte:Color4f, dpr:f32, last_id:usize) -> bool{
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

    pub fn update(&mut self, image:Option<SkImage>, page:&Page, matte:Color4f, dpr:f32, content:Rect){
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
