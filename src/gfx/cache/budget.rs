use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use crate::mem;

// maximum byte count across all stores and kinds (sized to fit ~32 concurrent 720p rasters)
const CACHE_BUDGET: u64 = 128 * 1024 * 1024;

// the uniform TTL for all three kinds
const CACHE_TTL: Duration = Duration::from_millis(5000);

// the current cache size (excluding pinned `willReadFrequently` surfaces), used for judging admission
pub static LIVE_BYTES: AtomicU64 = AtomicU64::new(0);

// SKIA_CANVAS_CACHE overrides the budget with a megabyte count or disables it with `off`/`0`/`false`
pub fn ceiling() -> u64{
  static BUDGET: OnceLock<u64> = OnceLock::new();
  *BUDGET.get_or_init(||{
    let setting = std::env::var("SKIA_CANVAS_CACHE")
      .map(|v| v.trim().to_ascii_lowercase())
      .unwrap_or_default();
    match setting.as_str(){
      "" => CACHE_BUDGET,
      "0" | "false" | "off" => 0,
      mb => mb.parse::<f64>().ok()
        .map(|mb| (mb.max(0.0) * 1048576.0) as u64)
        .unwrap_or(CACHE_BUDGET)
    }
  })
}

pub fn charge<V>(entry:&Entry<V>){
  if !entry.pinned{ LIVE_BYTES.fetch_add(entry.bytes, Ordering::Relaxed); }
}

pub fn credit<V>(entry:&Entry<V>){
  if !entry.pinned{
    LIVE_BYTES.fetch_update(Ordering::Relaxed, Ordering::Relaxed,
      |live| Some(live.saturating_sub(entry.bytes))).ok(); // clamp subtraction at 0
  }
}

// whether storing `headroom` more bytes would put the process over its budget
pub fn over_budget(headroom:u64) -> bool{
  LIVE_BYTES.load(Ordering::Relaxed) + headroom > ceiling()
}

// whether the ledger is currently above a given ceiling
pub fn above(ceiling:u64) -> bool{
  LIVE_BYTES.load(Ordering::Relaxed) > ceiling
}


//
// Entry: a bitmap wrapper with size and access tracking
//

pub struct Entry<V>{
  pub value: V,           // a raster of some sort (surface, export, or embed)
  pub last_read: Instant, // time of last access (idle entries are evicted first in sweeps)
  pub pinned: bool,       // exclude from sweeps if willReadFrequently is set (surfaces only)
  pub reads: u32,         // read-count since it was stored
  pub bytes: u64,         // byte-size of the raster
  #[allow(dead_code)]
  footprint: mem::v8::Footprint, // make v8 aware of the raster's byte size
}

// entries are ranked for eviction based on reads-per-MB (lower is worse) with staleness breaking ties
pub type Rank = (u64, Instant);

impl<V> Entry<V>{
  pub fn new(value:V, bytes:u64, reads:u32, pinned:bool) -> Self{
    Self{ value, bytes, footprint: mem::v8::Footprint::new(bytes as usize),
          last_read: Instant::now(), pinned, reads }
  }

  // track access count & recency for ranking during the next sweep
  pub fn touch(&mut self){
    self.reads += 1;
    self.last_read = Instant::now();
  }

  pub fn rank(&self) -> Rank{
    let density = match self.bytes{
      0 => u64::MAX, // evicting a zero-byte marker frees nothing, so never choose one
      bytes => (self.reads as u64).saturating_mul(1 << 20) / bytes,
    };
    (density, self.last_read)
  }

  pub fn expired(&self, now:Instant) -> bool{
    !self.pinned && now.duration_since(self.last_read) >= CACHE_TTL
  }
}

// the worst-ranked unpinned entry in a map, if it has one
pub fn worst_in<K:Copy, V>(map:&HashMap<K, Entry<V>>) -> Option<(K, Rank)>{
  map.iter()
    .filter(|(_, entry)| !entry.pinned)
    .min_by_key(|(_, entry)| entry.rank())
    .map(|(&key, entry)| (key, entry.rank()))
}

// remove all entries the predicate rejects, crediting the budget ledger for each
pub fn retain<K, V>(map:&mut HashMap<K, Entry<V>>, frees_heap:bool, mut keep:impl FnMut(&K, &Entry<V>) -> bool){
  let reclaimed:u64 = map
    .extract_if(|key, entry| !keep(key, entry))
    .map(|(_, entry)| { credit(&entry); entry.bytes })
    .sum();

  if frees_heap && reclaimed > 0{
    mem::glibc::mark_reclaimable(reclaimed as usize) // inform malloc_trim of the deallocations
  }
}

// remove a single entry and optionally report it to malloc_trim (i.e., only if it's CPU memory)
pub fn discard_one<V>(entry:Entry<V>, frees_heap:bool){
  credit(&entry);
  if frees_heap && entry.bytes > 0{
    mem::glibc::mark_reclaimable(entry.bytes as usize)
  }
}
