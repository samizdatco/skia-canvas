use std::cell::RefCell;
use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::gfx::page::RecordingSurface;

//
// SurfaceCache: persistent readback surfaces for `getImageData`, expired after TTL unless pinned via willReadFrequnetly
//

pub struct SurfaceCache;

thread_local!(
    // per-thread storage means CPU surfaces stay on the main thread and GPU on render_thread
    static SURFACES: RefCell<Surfaces> = RefCell::new(Surfaces::default());
);

impl SurfaceCache{
  // pass a RecordingSurface to the callback (creating it first if necessary)
  pub fn with_entry<T>(id:usize, read_frequently:bool, f:impl FnOnce(&mut RecordingSurface) -> T) -> T {
    SURFACES.with_borrow_mut(|state|{
      let now = Instant::now();
      let result = {
        let entry = state.map.entry(id).or_insert_with(|| Entry{
          surface: RecordingSurface::default(), bytes: 0, last_read: now, pinned: read_frequently,
        });
        entry.last_read = now; // mark as fresher than TTL (so it will survive the sweep below)
        let result = f(&mut entry.surface);
        entry.bytes = entry.surface.byte_size(); // the surface may have been (re)created in f
        result
      };
      state.sweep();
      result
    })
  }

  // evict for stale surfaces (called opportunistically from main thread)
  pub fn sweep(){ SURFACES.with_borrow_mut(|state| state.sweep()); }

  // evict the surface for a specific recorder (called from main thread for CPU, render_thread for GPU)
  pub fn evict(id:usize){ SURFACES.with_borrow_mut(|state|{ state.map.remove(&id); }) }

  // evict every live surface (called by the render thread just before it retires an idle GPU context)
  pub fn evict_all(){ SURFACES.with_borrow_mut(|state| state.map.clear()); }
}

#[derive(Default)]
struct Surfaces{
  map: HashMap<usize, Entry>,
}

const SURFACE_BUDGET: u64 = 128 * 1024 * 1024; // maximum total byte count for the whole cache
const SURFACE_TTL: Duration = Duration::from_millis(500); // maximum time since last access per-entry

struct Entry{
  surface: RecordingSurface,
  bytes: u64,         // surface byte-size
  last_read: Instant, // time of last read (idle surfaces are evicted first in sweeps)
  pinned: bool,       // exclude from sweeps if willReadFrequently is set
}

impl Surfaces{
  fn sweep(&mut self){
    // drop anything that's been idle longer than TTL
    let now = Instant::now();
    self.map.retain(|_, e| e.pinned || now.duration_since(e.last_read) < SURFACE_TTL);

    // keep dropping (stalest to freshest) until total size is below the budget
    let mut total: u64 = self.map.values().map(|e| e.bytes).sum();
    while total > SURFACE_BUDGET {
      let stalest = self.map.iter()
        .filter(|(_, e)| !e.pinned)
        .min_by_key(|(_, e)| e.last_read)
        .map(|(&id, e)| (id, e.bytes));
      match stalest{
        Some((id, bytes)) => { self.map.remove(&id); total -= bytes; }
        None => break, // only pinned surfaces remain
      }
    }
  }
}

//
// Frame: bitmap of the last on-screen render for a particular window (to be used as a backdrop for additive draw ops)
//

#[cfg(feature = "window")]
mod frame {
    use skia_safe::{Rect, Matrix, Color4f, Image as SkImage};
    use crate::gfx::page::Page;

    pub struct Frame {
        image: Option<SkImage>,
        content: Rect,
        page: Page,
        matte: Color4f,
        dpr: f32,
        state: FrameState,
    }

    impl Default for Frame{
        fn default() -> Self {
            Self{image:None, content:Rect::new_empty(), page:Page::default(), dpr:0.0, matte:Color4f::new(0.0, 0.0, 0.0, 0.0), state:FrameState::Clean}
        }
    }

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

    #[derive(Debug, PartialEq, Clone, Copy)]
    enum FrameState{
        Clean,
        Dirty,
        Resizing
    }
}

#[cfg(feature = "window")]
pub use frame::Frame;
