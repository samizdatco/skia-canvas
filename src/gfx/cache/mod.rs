use std::cell::RefCell;
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::Instant;

use skia_safe::{Color4f, ColorSpace, ISize, Image as SkImage};
use crate::gfx::page::{ExportOptions, Page};

mod budget;
mod rasters;
mod readback;
#[cfg(test)]
mod tests;

use budget::{above, ceiling, over_budget};
use rasters::{EmbedKey, Embeds, ExportKey, Exports};

pub use rasters::{may_snapshot, Snapshot};
pub use readback::ReadbackSurface;

//
// RasterConfig: the settings a cached raster was rendered with
//

// the export settings that can vary independently of a Page's content changing
#[derive(Clone, PartialEq)]
pub struct RasterConfig{
  pub density: f32,
  pub matte: Option<Color4f>,
  pub msaa: Option<usize>,
  pub color_space: ColorSpace,
}

impl RasterConfig{
  pub fn new(opts:&ExportOptions, color_space:&ColorSpace) -> Self{
    Self{ density: opts.density, matte: opts.matte, msaa: opts.msaa, color_space: color_space.clone() }
  }

  // combine the fields into a stable hash
  pub fn key(&self) -> u64{
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    let matte = self.matte.as_ref().map(|m| [m.r.to_bits(), m.g.to_bits(), m.b.to_bits(), m.a.to_bits()]);
    let space = (self.color_space.to_xyzd50_hash().0, self.color_space.transfer_fn_hash());
    (self.density.to_bits(), matte, self.msaa, space).hash(&mut h);
    h.finish()
  }
}

impl Default for RasterConfig{
  fn default() -> Self{
    Self{ density: 0.0, matte: None, msaa: None, color_space: ColorSpace::new_srgb() }
  }
}

//
// Residency: where a store's rasters physically live
//

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
enum Residency{
  #[cfg_attr(not(feature = "window"), allow(dead_code))]
  Window,       // textures owned by a window renderer's DirectContext
  RenderThread, // textures owned by the offscreen render thread's DirectContext
  SystemMemory, // ordinary CPU bitmaps, owned by no particular thread
}

impl Residency{
  // whether texture-backed rasters can be stored here
  fn admits_textures(&self) -> bool{
    *self != Residency::SystemMemory
  }

  // whether a particular image may be kept here (since it may be texture-backed)
  fn admits(&self, image:Option<&SkImage>) -> bool{
    self.admits_textures() || !image.is_some_and(|img| img.is_texture_backed())
  }

  // whether evictions need to be posted to the render_thread
  fn needs_posted_eviction(&self) -> bool{
    *self == Residency::RenderThread
  }

  // whether freed memory should be reported to malloc_trim (since it comes from the system heap)
  fn frees_heap(&self) -> bool{
    *self == Residency::SystemMemory
  }
}


//
// Store: the export and embed rasters for a particular residency
//

struct Store{
  exports: Exports,
  embeds: Embeds,
  residency: Residency,
}

impl Store{
  fn new(residency:Residency) -> Self{
    Self{ exports: Exports::new(residency), embeds: Embeds::new(residency), residency }
  }

  // drop every raster derived from a particular page recording
  fn evict(&mut self, page:usize){
    self.exports.evict(page);
    self.embeds.evict(page);
  }

  fn clear(&mut self){
    self.exports.clear();
    self.embeds.clear();
  }

  // whether anything here is holding real memory (a zero-byte export marker is not)
  fn holds_live_rasters(&self) -> bool{
    self.exports.holds_live_rasters() || self.embeds.holds_live_rasters()
  }

  // evict entries to try to get to the budget target
  fn sweep(&mut self){
    self.sweep_within(ceiling())
  }

  // evict entries to try to get below the requested ceiling
  fn sweep_within(&mut self, ceiling:u64){
    // expire anything staler than TTL
    let now = Instant::now();
    self.exports.expire(now);
    self.embeds.expire(now);
    readback::expire(now);

    // while over budget, keep dropping the worst-ranked entry that's reachable from this residency
    while above(ceiling){
      enum Slot{ Export(ExportKey), Embed(EmbedKey), Readback(usize) }

      let candidates = [
        self.exports.worst().map(|(key, rank)| (rank, Slot::Export(key))),
        self.embeds.worst().map(|(key, rank)| (rank, Slot::Embed(key))),
        readback::worst().map(|(page, rank)| (rank, Slot::Readback(page))),
      ];

      match candidates.into_iter().flatten().min_by_key(|(rank, _)| *rank){
        Some((_, Slot::Export(key))) => self.exports.discard(key),
        Some((_, Slot::Embed(key))) => self.embeds.discard(key),
        Some((_, Slot::Readback(page))) => readback::discard(page),
        None => break, // nothing reachable from here is evictable
      }
    }
  }
}

impl Drop for Store{
  // credit this store's bytes back to the ledger (e.g. after a window renderer is dropped)
  fn drop(&mut self){
    self.clear();
  }
}

//
// The two places a Store can live
//

// the shared store for CPU-backed rasters (its residency rejects texture-backed images)
pub struct SharedStore(Mutex<Store>);
unsafe impl Send for SharedStore{}
unsafe impl Sync for SharedStore{}

impl SharedStore{
  fn lock(&self) -> MutexGuard<'_, Store>{
    self.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
  }
}

// a GPU-backed store whose rasters are tied to the DirectContext that created them.
// intentionally not Send or Sync to ensure it doesn't leave its original thread
pub struct DeviceStore(RefCell<Store>);

impl DeviceStore{
  pub fn for_render_thread() -> Self{
    Self(RefCell::new(Store::new(Residency::RenderThread)))
  }

  #[cfg_attr(not(feature = "window"), allow(dead_code))]
  pub fn for_window() -> Self{
    Self(RefCell::new(Store::new(Residency::Window)))
  }
}

//
// Cache: the public api used during rendering (ensures the correct Store is backing it)
//

#[derive(Clone, Copy)]
pub enum Cache<'a>{
  Device(&'a DeviceStore), // can be initialized with a store reference
  Shared(&'static SharedStore), // can only be accessed via Cache::shared()
}

impl Cache<'static>{
  // returns the shared cache for CPU rasters
  pub fn shared() -> Self{
    static SHARED: OnceLock<SharedStore> = OnceLock::new();
    Cache::Shared(SHARED.get_or_init(|| SharedStore(Mutex::new(Store::new(Residency::SystemMemory)))))
  }
}

impl Cache<'_>{
  fn with<T>(&self, f:impl FnOnce(&mut Store) -> T) -> T{
    match self{
      // whichever variant this is, borrow the underlying store and pass it to the callback
      Cache::Device(store) => f(&mut store.0.borrow_mut()),
      Cache::Shared(store) => f(&mut store.lock()),
    }
  }

  // access the readback surface for the specified page
  pub fn readback<T>(&self, page:usize, read_frequently:bool, f:impl FnOnce(&mut ReadbackSurface) -> T) -> T{
    let (mut surface, pinned, reads) = readback::take(page, read_frequently); // pull the surface out (or create one)
    let result = f(&mut surface); // let the callback use it
    readback::put(page, surface, pinned, reads); // put the surface back in
    self.sweep();
    result
  }

  // access/update the export snapshot for the specified page and raster settings
  pub fn export<T>(&self, page:&Page, opts:&ExportOptions, color_space:&ColorSpace,
                   f:impl FnOnce(&mut Snapshot, &RasterConfig) -> T) -> T{
    let config = RasterConfig::new(opts, color_space);
    let key = (page.id, config.key());

    // pull the existing snapshot out (or start with an empty, unread one)
    let (mut snapshot, prior_reads) = self.with(|store| store.exports.take(&key)).unwrap_or_default();

    // let the callback use it
    let result = f(&mut snapshot, &config);

    // detect whether the callback wrote to it (and wipe the preexisting read-count if so)
    let reads = match std::mem::take(&mut snapshot.replaced){ true => 0, false => prior_reads };

    // if snpashot has a texture-backed raster, record that it will need to be dropped on the render thread
    if snapshot.image.is_some() && self.needs_posted_eviction(){ page.evict_on_render_thread() }

    // put the snapshot back in
    self.with(|store|{
      store.exports.put(key, snapshot, reads);
      store.sweep();
    });
    result
  }

  // access/update the rasterized canvas for this page at a particular size, placement, and layer depth
  pub fn embed(&self, page:&Page, placement:u64, dims:ISize, cost:u64,
               f:impl FnOnce(Option<(&SkImage, usize)>) -> Option<SkImage>) -> Option<SkImage>{
    let stamp = page.stamp();

    // if there's an exact match covering every layer, we're done
    let exact = EmbedKey::new(stamp.id, placement, stamp.epoch, stamp.depth);
    if let Some(image) = self.with(|store| store.embeds.read(exact, dims)){
      return Some(image)
    }

    // otherwise, start by sweeping to see if the cache is already full (in which case, bail out),
    // then look for an incomplete match with an earlier slice of layers to draw on top of
    let base = self.with(|store|{
      store.sweep();
      (!over_budget(cost)).then(|| store.embeds.read_base(stamp, placement, dims))
    })?;

    // pass the base to the callback so it can fill in the more recent layers (bail if it doesn't)
    let image = f(base.as_ref().map(|(image, covered)| (image, *covered)))?;

    // add the updated raster to the cache (and plan its disposal) before returning it
    self.with(|store| store.embeds.put(stamp, placement, image.clone(), cost));
    if self.needs_posted_eviction(){ page.evict_on_render_thread() }
    Some(image)
  }

  // whether a page whose rasters landed here must post its eviction to the render_thread
  fn needs_posted_eviction(&self) -> bool{
    self.with(|store| store.residency.needs_posted_eviction())
  }

  // evict stale entries (called opportunistically and after every render job)
  pub fn sweep(&self){
    self.with(|store| store.sweep())
  }

  // drop every raster derived from a particular page recording
  pub fn evict(&self, page:usize){
    self.with(|store| store.evict(page));
    readback::discard(page);
  }

  // whether this store has anything worth keeping a GPU context alive for
  pub fn holds_live_rasters(&self) -> bool{
    self.with(|store| store.holds_live_rasters()) || readback::holds_live_surfaces()
  }

  // drop everything reachable from here (called just before a GPU context is retired)
  pub fn clear(&self){
    self.with(|store| store.clear());
    readback::clear();
  }
}
