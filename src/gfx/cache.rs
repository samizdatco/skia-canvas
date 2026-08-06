// Caches for gpu-backed (and gpu-adjacent) page resources. This module owns the storage,
// render-thread lifetime, and eviction policy for the backends' cached page rasters:
//
//   • SurfaceCache — live getImageData surfaces (CPU- or GPU-backed), keyed by PageRecorder id,
//     bounded by a byte budget (B, access-time LRU) plus an idle timeout (T) cull; willReadFrequently
//     pins against both. Backed by a single thread_local, so each thread that reads pixels gets its
//     own map: CPU reads populate the JS thread's map (inline), GPU reads populate the render
//     thread's (inside engine.render). GPU surfaces are !Send and never leave the render thread.
//
//   • Frame — the last on-screen raster a windowed renderer painted, held as a single slot per
//     renderer (not a keyed lookup) and reused as the backdrop the next frame composites onto.
//
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

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
// SurfaceCache: live getImageData surfaces (CPU- or GPU-backed), keyed by PageRecorder id, bounded
// by a byte budget (B, access-time LRU) plus an idle timeout (T) cull. willReadFrequently pins
// against both. B/T are env-overridable for memory-constrained deployments.
//

// byte budget (MB via SKIA_CANVAS_SURFACE_BUDGET): evict least-recently-read unpinned surfaces past
// it. sized to cover a realistic reuse working set (single full-frame … ~32 concurrent 720p).
fn budget_bytes() -> u64 {
  static BUDGET: OnceLock<u64> = OnceLock::new();
  *BUDGET.get_or_init(|| std::env::var("SKIA_CANVAS_SURFACE_BUDGET").ok()
    .and_then(|v| v.parse::<u64>().ok()).unwrap_or(128) * 1024 * 1024)
}

// idle timeout (ms via SKIA_CANVAS_SURFACE_TTL): cull unpinned surfaces not read within it, so a
// warm surface survives active reads + a brief pause but is reclaimed once the reader stops.
fn ttl() -> Duration {
  static TTL: OnceLock<Duration> = OnceLock::new();
  *TTL.get_or_init(|| Duration::from_millis(std::env::var("SKIA_CANVAS_SURFACE_TTL").ok()
    .and_then(|v| v.parse::<u64>().ok()).unwrap_or(500)))
}

struct Entry{
  surface: RecordingSurface,
  bytes: u64,          // surface byte-size, summed against the budget
  last_read: Instant,  // wall-clock of last read, for access-time LRU + idle-T cull
  pinned: bool,        // willReadFrequently → exempt from eviction
}

#[derive(Default)]
struct Surfaces{
  map: HashMap<usize, Entry>,
}

impl Surfaces{
  // reclaim unpinned surfaces: first cull anything idle longer than T, then evict least-recently-read
  // until total bytes are under B. dropping an Entry frees the skia surface (and its native
  // accounting); for the render thread's map that happens inside engine.render — the only place a
  // gpu surface may drop.
  fn sweep(&mut self){
    let (now, ttl) = (Instant::now(), ttl());
    self.map.retain(|_, e| e.pinned || now.duration_since(e.last_read) < ttl);

    let budget = budget_bytes();
    let mut total: u64 = self.map.values().map(|e| e.bytes).sum();
    while total > budget {
      let victim = self.map.iter()
        .filter(|(_, e)| !e.pinned)
        .min_by_key(|(_, e)| e.last_read)
        .map(|(&id, e)| (id, e.bytes));
      match victim{
        Some((id, bytes)) => { self.map.remove(&id); total -= bytes; }
        None => break, // only pinned surfaces remain: the budget yields to willReadFrequently
      }
    }
  }
}

thread_local!(
    // per-thread live surfaces: the JS thread holds CPU-backed entries (getImageData is synchronous),
    // the render thread holds GPU-backed entries (touched only inside engine.render/render_soon jobs).
    // never shared, so !Send gpu surfaces stay render-thread-bound and no locking is needed.
    static SURFACES: RefCell<Surfaces> = RefCell::new(Surfaces::default());
);

pub struct SurfaceCache;

impl SurfaceCache{
  // run `f` against the surface for `id` (creating it if absent), refresh its recency + pin state
  // and byte-size, then sweep. call on the surface's owning thread: the JS thread for CPU reads,
  // the render thread (inside engine.render) for GPU reads.
  pub fn with_entry<T>(id:usize, read_frequently:bool, f:impl FnOnce(&mut RecordingSurface) -> T) -> T {
    SURFACES.with_borrow_mut(|state|{
      let now = Instant::now();
      let result = {
        let entry = state.map.entry(id).or_insert_with(|| Entry{
          surface: RecordingSurface::default(), bytes: 0, last_read: now, pinned: read_frequently,
        });
        entry.last_read = now;          // most-recently-read → never the victim in sweep below
        entry.pinned = read_frequently; // a canvas may re-declare the hint between reads
        let result = f(&mut entry.surface);
        entry.bytes = entry.surface.byte_size(); // the surface may have been (re)created in f
        result
      };
      state.sweep();
      result
    })
  }

  // sweep the current thread's map. called from Canvas creation, exports, and dispose — the JS
  // thread's activity points, since it lacks the render thread's own idle sweeper.
  pub fn sweep(){
    SURFACES.with_borrow_mut(|state| state.sweep());
  }

  // evict the surface for a single recorder (its PageRecorder is being released). call on the
  // owning thread: inline for a CPU surface, via render_soon for a GPU surface.
  pub fn evict(id:usize){
    SURFACES.with_borrow_mut(|state|{ state.map.remove(&id); })
  }

  // evict every surface on this thread, pinned or not: the render thread calls this just before it
  // retires an idle gpu context — the surfaces are bound to that context, so the pin can't save them.
  pub fn evict_all(){
    SURFACES.with_borrow_mut(|state| state.map.clear());
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
