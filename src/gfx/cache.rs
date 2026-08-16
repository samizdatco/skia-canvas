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
