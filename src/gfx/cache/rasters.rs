use std::collections::HashMap;
use std::time::Instant;

use skia_safe::{ISize, Image as SkImage};
use crate::gfx::page::{Page, PageVersion};

use super::{RasterConfig, Residency};
use super::budget::{charge, credit, discard_one, retain, worst_in, Entry, Rank};

//
// Exports: one snapshot per (page, raster-config)
//

pub type ExportKey = (usize, u64); // (page id, RasterConfig::key)

pub struct Exports{
  pub(super) map: HashMap<ExportKey, Entry<Snapshot>>,
  residency: Residency,
}

impl Exports{
  pub fn new(residency:Residency) -> Self{
    Self{ map: HashMap::new(), residency }
  }

  // lend a snapshot to a caller, leaving the map empty until `put` replaces it
  pub fn take(&mut self, key:&ExportKey) -> Option<(Snapshot, u32)>{
    let mut entry = self.map.remove(key)?;
    credit(&entry); // remove its bytes from the total
    entry.touch(); // increment the read count
    Some((entry.value, entry.reads)) // the count rides along so `put` can restore it
  }

  pub fn put(&mut self, key:ExportKey, snapshot:Snapshot, reads:u32){
    // decline rather than retain a texture if its context can't be reached to drop it
    if !self.residency.admits(snapshot.image.as_ref()){ return }
    let bytes = snapshot.bytes; // read before the move; an export slot is never pinned
    let entry = Entry::new(snapshot, bytes, reads, false);
    charge(&entry); // add the bytes back to the total
    if let Some(old) = self.map.insert(key, entry){
      discard_one(old, self.residency.frees_heap()) // an insert can displace a stale snapshot
    }
  }

  //
  // lifecycle methods for cache sweeps
  //

  pub fn expire(&mut self, now:Instant){
    retain(&mut self.map, self.residency.frees_heap(), |_, entry| !entry.expired(now))
  }

  pub fn worst(&self) -> Option<(ExportKey, Rank)>{
    worst_in(&self.map)
  }

  pub fn discard(&mut self, key:ExportKey){
    retain(&mut self.map, self.residency.frees_heap(), |k, _| *k != key)
  }

  pub fn evict(&mut self, page:usize){
    retain(&mut self.map, self.residency.frees_heap(), |(id, _), _| *id != page)
  }

  pub fn clear(&mut self){
    retain(&mut self.map, self.residency.frees_heap(), |_, _| false)
  }

  // whether anything here is holding real memory (a zero-byte export marker is not)
  pub fn holds_live_rasters(&self) -> bool{
    self.map.values().any(|entry| entry.bytes > 0)
  }
}


//
// Pages: one raster per (page version, placement)
//

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub struct PageKey{
  pub(super) version: PageVersion, // page id, bounds revision, and how many layers it incorporates
  pub(super) placement: u64,       // quantized scale + sub-pixel phase (see `Page::resolve`)
}

pub struct Pages{
  pub(super) map: HashMap<PageKey, Entry<SkImage>>,
  residency: Residency,
}

impl Pages{
  pub fn new(residency:Residency) -> Self{
    Self{ map: HashMap::new(), residency }
  }

  // read a raster and update its access count
  pub fn read(&mut self, key:PageKey, size:ISize) -> Option<SkImage>{
    let entry = self.map.get_mut(&key)?; // compare based on quantized scale (1/64 buckets)
    if entry.value.dimensions() != size{ return None } // double-check the true size matches
    entry.touch(); // mark it used (to prioritize it in future sweeps)
    Some(entry.value.clone())
  }

  // the deepest raster for this page+placement that covers only an *earlier* slice of its layers,
  // so the remaining ones can be drawn on top of it
  pub fn read_base(&mut self, version:PageVersion, placement:u64, size:ISize) -> Option<(SkImage, usize)>{
    // `extends` tests whether it can be built upon & depth makes sure it's an ancestor
    let base = self.map.keys()
      .filter(|key| key.placement == placement
                    && key.version.extends(&version) && key.version.depth < version.depth)
      .max_by_key(|key| key.version.depth)
      .copied()?;
    self.read(base, size).map(|image| (image, base.version.depth))
  }

  pub fn put(&mut self, version:PageVersion, placement:u64, image:SkImage, bytes:u64){
    if !self.residency.admits(Some(&image)){ return } // decline rather than retain a texture

    // drop any raster of the same page and placement that this one supersedes
    retain(&mut self.map, self.residency.frees_heap(), |key, _|
      !(key.version.id == version.id && key.placement == placement
        && (key.version.epoch != version.epoch || key.version.depth < version.depth))
    );

    let entry = Entry::new(image, bytes, 0, false);
    charge(&entry);
    if let Some(old) = self.map.insert(PageKey{ version, placement }, entry){
      // the retain above spares an entry at this exact slot, which a re-store then displaces
      discard_one(old, self.residency.frees_heap())
    }
  }

  //
  // the maintenance surface shared by all three kinds
  //

  pub fn expire(&mut self, now:Instant){
    retain(&mut self.map, self.residency.frees_heap(), |_, entry| !entry.expired(now))
  }

  pub fn worst(&self) -> Option<(PageKey, Rank)>{
    worst_in(&self.map)
  }

  pub fn discard(&mut self, key:PageKey){
    retain(&mut self.map, self.residency.frees_heap(), |k, _| *k != key)
  }

  pub fn evict(&mut self, page:usize){
    retain(&mut self.map, self.residency.frees_heap(), |key, _| key.version.id != page)
  }

  pub fn clear(&mut self){
    retain(&mut self.map, self.residency.frees_heap(), |_, _| false)
  }

  pub fn holds_live_rasters(&self) -> bool{
    self.map.values().any(|entry| entry.bytes > 0)
  }
}


//
// Snapshot: the value an export entry holds
//

// imperfect conservative threshold to avoid wasting memory/crowding out cache on quick-to-render shallow
// canvases. it fails to spot pages with heavy drawing in a shallow layer stack, but no tested predicate
// performed better overall (display list weight, render duration, & raster dimensions were tried).
const MIN_SNAPSHOT_DEPTH: usize = 8;

// whether a page this deep could possibly end up creating a snapshot
pub fn may_snapshot(depth:usize) -> bool{
  depth >= MIN_SNAPSHOT_DEPTH
}

#[derive(Default)]
pub struct Snapshot{
  pub image: Option<SkImage>,  // the raster (may be texture-backed)
  pub version: PageVersion,    // the page state (id, bounds epoch, layer count) captured by `image`
  config: RasterConfig,        // the raster settings `image` was rendered with
  pub(super) bytes: u64,       // the size
  seen: bool,                  // has this (page, config) exported before?
  pub(super) replaced: bool,   // did the last borrower call `store`?
}

impl Snapshot{
  // whether `image` can be used as a base for additional drawing by this page + config
  pub fn accepts(&self, page:&Page, config:&RasterConfig) -> bool{
    self.image.is_some() &&
    self.config == *config &&
    self.version.extends(&page.version())
  }

  // caching exports is gated by a store-on-second-export rule
  pub fn recur(&mut self) -> bool{
    let seen = self.seen;
    self.seen = true;
    seen
  }

  // replace the raster this slot holds
  pub fn store(&mut self, image:Option<SkImage>, page:&Page, config:&RasterConfig, bytes:u64){
    self.image = image;
    self.version = page.version();
    self.config = config.clone();
    self.bytes = bytes;
    self.replaced = true; // flag for Cache::export that the borrower wrote rather than read
  }
}
