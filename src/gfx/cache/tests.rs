//! The sweep's eviction order, the ledger's bookkeeping, and the store routing are all
//! invisible to the pixel-level suite — evictions never change output, only who pays to
//! rebuild — so they are pinned here, where entries can be laid out directly.
//!
//! The ledger is process-global, so these serialize on a lock and each builds its stores from
//! empty (a `Store` credits everything back on drop, leaving the ledger where it found it).

use std::sync::{Mutex, MutexGuard};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use skia_safe::{ColorSpace, ISize, Image as SkImage};
use crate::gfx::page::{ExportOptions, Page, PageVersion};

use super::*;
use super::budget::{charge, Entry, LIVE_BYTES};
use super::rasters::{PageKey, Snapshot};
use super::readback;

static LEDGER_LOCK: Mutex<()> = Mutex::new(());
fn serialized() -> MutexGuard<'static, ()>{
  // each test starts from an empty thread-local readback map as well as empty stores
  readback::clear();
  LEDGER_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

const MB: u64 = 1024 * 1024;

fn image() -> SkImage{
  skia_safe::surfaces::raster_n32_premul((4, 4)).unwrap().image_snapshot()
}

// Ages stay under the TTL so the budget loop, not expiry, decides who goes; `reads` is what
// the victim score keys on, so most cases want to set it explicitly.
fn put_export(store:&mut Store, page:usize, bytes:u64, age_ms:u64, reads:u32){
  let mut entry = Entry::new(Snapshot::default(), bytes, reads, false);
  entry.last_read = Instant::now() - Duration::from_millis(age_ms);
  charge(&entry);
  store.exports.map.insert((page, 0), entry);
}

fn put_page(store:&mut Store, page:usize, bytes:u64, age_ms:u64, reads:u32){
  let mut entry = Entry::new(image(), bytes, reads, false);
  entry.last_read = Instant::now() - Duration::from_millis(age_ms);
  charge(&entry);
  store.pages.map.insert(PageKey{ page, placement:0, epoch:0, depth:1 }, entry);
}

fn has_export(store:&Store, page:usize) -> bool{ store.exports.map.contains_key(&(page, 0)) }
fn has_page(store:&Store, page:usize) -> bool{
  store.pages.map.contains_key(&PageKey{ page, placement:0, epoch:0, depth:1 })
}
fn count(store:&Store) -> usize{ store.exports.map.len() + store.pages.map.len() }

#[test]
fn eviction_prefers_reuse_density_over_size_and_kind(){
  let _lock = serialized();
  let mut store = Store::new(Residency::SystemMemory);
  // The case the shipped ordering exists for, and the one the previous per-kind order got
  // backwards: a big export snapshot nothing has read against a small page raster serving
  // every frame. Kind ranking kept the snapshot (its rebuild costs more *per entry*) and took
  // the raster; measured on real workloads that cost 15.7x. Reuse density takes the snapshot.
  put_export(&mut store, 1, 8*MB, 0, 0);
  put_page(&mut store, 2, 1*MB, 0, 200);

  store.sweep_within(8*MB);
  assert!(has_page(&store, 2), "a raster serving every frame was evicted to keep an unread snapshot");
  assert_eq!(count(&store), 1);
}

#[test]
fn a_zero_byte_marker_is_never_a_budget_victim(){
  let _lock = serialized();
  let mut store = Store::new(Residency::SystemMemory);
  // evicting one frees nothing, so choosing it would spin the loop without making progress.
  // NB the age stays under the TTL or expiry, not the budget loop, would decide this.
  put_export(&mut store, 1, 0, 1000, 0);
  put_page(&mut store, 2, 4*MB, 0, 0);

  store.sweep_within(1*MB);
  assert!(has_export(&store, 1), "the zero-byte marker was chosen as a victim");
  assert_eq!(count(&store), 1);
}




#[test]
fn the_ledger_spans_stores_and_is_credited_on_drop(){
  let _lock = serialized();
  let baseline = LIVE_BYTES.load(Ordering::Relaxed);
  let mut resident = Store::new(Residency::SystemMemory);
  put_page(&mut resident, 1, 6*MB, 0, 0);
  {
    let mut window = Store::new(Residency::Window);
    put_page(&mut window, 2, 6*MB, 0, 0);
    readback::seed(3, 6*MB, true, 0);
    // 12 MB unpinned across the two stores (the pinned 6 MB never enters the ledger)
    assert_eq!(LIVE_BYTES.load(Ordering::Relaxed) - baseline, 12*MB);

    // pressure against the shared total can only evict what the sweeper can reach: it gives up
    // its own entry, then breaks — the other store's bytes stay where their owner put them
    window.sweep_within(4*MB);
    assert_eq!(count(&window), 0, "only the window's own unpinned entry was reachable");
    assert_eq!(LIVE_BYTES.load(Ordering::Relaxed) - baseline, 6*MB);
    readback::clear();
  } // a dropped store (a closing window) credits everything back
  assert_eq!(LIVE_BYTES.load(Ordering::Relaxed) - baseline, 6*MB);
  drop(resident);
  assert_eq!(LIVE_BYTES.load(Ordering::Relaxed), baseline);
}


#[test]
fn only_a_live_raster_vetoes_retirement(){
  let _lock = serialized();
  let device = DeviceStore::for_render_thread();
  let cache = Cache::Device(&device);
  assert!(!cache.holds_live_rasters(), "an empty store has nothing to keep a context alive for");

  // a first-export marker holds no memory, so it is not worth deferring retirement for
  put_export(&mut device.0.borrow_mut(), 1, 0, 0, 0);
  assert!(!cache.holds_live_rasters(), "a zero-byte marker must not veto");

  // …a raster does
  put_page(&mut device.0.borrow_mut(), 2, 4*MB, 0, 0);
  assert!(cache.holds_live_rasters(), "a cached raster must veto retirement");

  // …and so does a pinned readback surface, which is the case the ledger cannot see: its bytes
  // are excluded from LIVE_BYTES, but the surface is live and retiring would drop it
  device.0.borrow_mut().clear();
  assert!(!cache.holds_live_rasters());
  readback::seed(3, 4*MB, true, 0);
  assert_eq!(LIVE_BYTES.load(Ordering::Relaxed), 0, "a pinned entry stays out of the ledger");
  assert!(cache.holds_live_rasters(), "a pinned readback surface must veto retirement");
  readback::clear();
}

#[test]
fn a_sweep_takes_victims_in_rank_order_across_all_three_kinds(){
  let _lock = serialized();
  let mut store = Store::new(Residency::SystemMemory);

  // one candidate per rule, in one pool: `a` is the least reused, `b` and `c` tie on reuse
  // density so recency separates them, and `d` is pinned and must never be chosen at all
  put_export(&mut store, 1, 8*MB, 0, 0);              // a: density 0 — lowest
  put_page  (&mut store, 2, 4*MB, 2000, 4);           // b: density 1, stale
  put_page  (&mut store, 3, 4*MB, 0, 4);              // c: density 1, fresh
  readback::seed(4, 8*MB, true, 0); // d: pinned

  // 16 MB unpinned; a ceiling of 4 forces exactly two evictions
  store.sweep_within(4*MB);
  assert!(!has_export(&store, 1), "the least-reused entry goes first, whatever its kind");
  assert!(!has_page(&store, 2), "of two entries tied on reuse density, the staler goes");
  assert!(has_page(&store, 3), "…and the fresher survives");
  assert!(readback::holds_live_surfaces(), "a pinned surface is never a candidate");

  // nothing can satisfy a ceiling of 0 once only the pinned surface is left: the loop must
  // drain what it can and then break rather than spin (a regression here hangs the test)
  store.sweep_within(0);
  assert_eq!(count(&store), 0, "the last unpinned entry is still evictable");
  assert!(readback::holds_live_surfaces(), "and the pinned one still isn't");
  readback::clear();
}

#[test]
fn the_shared_store_is_process_wide_and_device_stores_are_not(){
  let _lock = serialized();
  let version = PageVersion{ id: 9, epoch: 0, depth: 1 };
  let dims = ISize::new(4, 4);

  // The shared store is what a CPU pass on any thread is handed, so a slot touched by one thread
  // has to be visible from another — which is what an async export sequence does across the rayon
  // pool. Checked with the export slot's second-export marker, since that is the state whose
  // visibility actually decides whether a snapshot gets kept.
  let (opts, cs) = (ExportOptions::default(), ColorSpace::new_srgb());
  Cache::shared().export(&Page::default(), &opts, &cs, |snap, _|{
    assert!(!snap.recur(), "a fresh slot: the first export only leaves the marker");
  });
  std::thread::scope(|scope|{
    scope.spawn(||{
      Cache::shared().export(&Page::default(), &opts, &cs, |snap, _|{
        assert!(snap.recur(), "…and the marker must be visible from another thread");
      });
    });
  });
  Cache::shared().evict(0); // `Page::default()` is id 0

  // The same claim for page rasters, asked of the map directly rather than through `Cache::page_raster` —
  // which reports a reuse and a fresh build as the same `Option`, and would insert on the miss.
  // Seeding goes straight to the store for the same reason: what is being tested is which store
  // an entry landed in, not the admission path that put it there.
  let key = PageKey::new(version.id, 0, version.epoch, version.depth);
  let holds = |cache:Cache| cache.with(|store| store.pages.read(key, dims).is_some());
  let seed = |cache:Cache| cache.with(|store| store.pages.put(version, 0, image(), MB));

  seed(Cache::shared());
  assert!(holds(Cache::shared()));

  // a device store is reachable only through its own handle, in either direction
  let device = DeviceStore::for_window();
  let other = DeviceStore::for_window();
  seed(Cache::Device(&device));
  assert!(!holds(Cache::Device(&other)),
          "a device store's rasters must not leak to another device store");

  Cache::shared().evict(9);
  assert!(holds(Cache::Device(&device)),
          "…nor may evicting the shared store reach into one");
}

#[test]
fn an_export_slot_counts_reads_until_its_raster_is_replaced(){
  let _lock = serialized();
  // The count is what `Entry::rank` weighs a slot's reuse density by, and the only path that
  // advances it is `Exports::take` — which the seeding helpers above deliberately bypass. So this
  // is the one case that exercises a full round trip through `Cache::export`.
  let (page, opts, cs) = (Page::default(), ExportOptions::default(), ColorSpace::new_srgb());
  let key = (page.id, RasterConfig::new(&opts, &cs).key());
  let cache = Cache::shared();
  let reads = || cache.with(|store| store.exports.map.get(&key).map(|entry| entry.reads));

  // a miss leaves nothing but the second-export marker, unread
  cache.export(&page, &opts, &cs, |snap, _| assert!(!snap.recur()));
  assert_eq!(reads(), Some(0), "a slot created by a miss starts at zero");

  // every borrow that only reads counts toward keeping the slot through the next sweep
  cache.export(&page, &opts, &cs, |snap, _| assert!(snap.recur()));
  assert_eq!(reads(), Some(1), "a read must advance the count");
  cache.export(&page, &opts, &cs, |_, _| ());
  assert_eq!(reads(), Some(2));

  // …until a borrower replaces the raster, whose own reuse has to be earned from scratch
  cache.export(&page, &opts, &cs, |snap, config| snap.store(Some(image()), &page, config, MB));
  assert_eq!(reads(), Some(0), "a replacement inherited the previous raster's read count");

  cache.export(&page, &opts, &cs, |_, _| ());
  assert_eq!(reads(), Some(1), "…and then counts its own");

  cache.evict(0); // `Page::default()` is id 0
}
