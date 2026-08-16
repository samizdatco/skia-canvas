use std::cell::RefCell;
use std::collections::HashMap;
use std::time::Instant;

use skia_safe::{Color, IRect, ImageInfo, Surface};
use crate::gfx::page::{Page, PageStamp, ExportOptions, Replay};
use crate::gfx::RenderingEngine;
use crate::mem;

use super::RasterConfig;
use super::budget::{charge, credit, worst_in, Entry, Rank};

//
// The surface `getImageData` rasterizes into then reads back from
//

pub struct ReadbackSurface{
  surface: Option<Surface>,
  stamp: PageStamp, // identify the page layers already included in the surface's raster
  gpu: Option<bool>,
  config: RasterConfig,
}

impl Default for ReadbackSurface{
  fn default() -> Self {
    Self{surface:None, stamp:PageStamp::default(), gpu:None, config:RasterConfig::default()}
  }
}

impl ReadbackSurface{
  fn is_surface_stale(&mut self, page:&Page, opts:&ExportOptions, engine:&RenderingEngine) -> bool{
    let gpu_toggled = self.gpu != Some(matches!(engine, RenderingEngine::GPU));
    let page_size = page.scaled_dimensions(opts.density);
    let resized = self.surface.as_mut().map(|surface|{
      surface.image_info().dimensions() != page_size
    }).unwrap_or(true);

    gpu_toggled || resized
  }

  pub fn update(&mut self, page:&Page, opts:&ExportOptions, engine:&RenderingEngine, cache:super::Cache){
    let color_space = opts.color_space.clone().unwrap_or_else(|| page.color_space.clone());
    let config = RasterConfig::new(opts, &color_space);

    // check whether the existing surface raster is still valid as a base for new layering
    let reconfigure = self.config != config;
    let recreate = self.is_surface_stale(&page, &opts, &engine);
    let restart = reconfigure || recreate || !self.stamp.extends(&page.stamp());

    // start from scratch if invalidated
    if restart{
      self.gpu = Some(matches!(engine, RenderingEngine::GPU));
      self.config = config;
      self.stamp = PageStamp{ depth:0, ..page.stamp() };

      // only allocate a new surface if the dimensions (size * density) have changed or engine switched
      if recreate{
        let page_size = page.scaled_dimensions(opts.density);
        let img_info = ImageInfo::new_n32_premul(page_size, color_space.clone());
        let budgeted = false; // the readback cache owns this, so keep it out of skia's glyph/texture/scratch budget
        self.surface = engine.make_surface(&img_info, &opts, budgeted).ok();
      }
    }

    if let Some(surface) = self.surface.as_mut(){
      let canvas = surface.canvas();

      // fill a fresh/recreated surface with the matte; a persistent surface keeps its prior contents
      // and just replays the layers added since the last update
      if self.stamp.depth==0 {
        canvas.clear(self.config.matte.unwrap_or(Color::TRANSPARENT.into()));
      }

      // draw newly added layers
      canvas.scale((self.config.density, self.config.density));
      page.playback_from(canvas, self.stamp.depth, None, Replay::Raster(cache));
      self.stamp = page.stamp();
    }
  }

  pub fn copy_pixels(&mut self, dst_info: &ImageInfo, src: IRect, pixels: &mut [u8]) -> bool{
    self.surface.as_mut().map(|surface|{
      surface.read_pixels(dst_info, pixels, dst_info.min_row_bytes(), (src.x(), src.y()))
    }).unwrap_or(false)
  }

  // logical byte-size of the current surface (w×h×4×density²), 0 if none — for the cache budget
  pub fn byte_size(&mut self) -> u64{
    self.surface.as_mut().map(|s| s.image_info().compute_min_byte_size() as u64).unwrap_or(0)
  }

  // whether this surface's pixels came from the driver rather than the heap (so freeing it is
  // not something malloc_trim should be told about)
  pub fn is_gpu_backed(&self) -> bool{
    self.gpu == Some(true)
  }
}

// surfaces are not routed to a Store like other rasters but kept in a hashmap on the thread that
// created them (the main JS thread for CPU / render_thread for GPU) and are keyed by Page id

thread_local!(
  static SURFACES: RefCell<HashMap<usize, Entry<ReadbackSurface>>> =
    RefCell::new(HashMap::new());
);

// lend a surface to a caller, leaving the map empty until `put` replaces it
pub fn take(page:usize, pinned:bool) -> (ReadbackSurface, bool, u32){
  SURFACES.with_borrow_mut(|surfaces| match surfaces.remove(&page){
    Some(mut entry) => {
      credit(&entry); // the caller now holds the bytes; `put` re-charges them
      entry.touch();
      (entry.value, entry.pinned, entry.reads) // an existing entry keeps its pinning
    }
    None => (ReadbackSurface::default(), pinned, 0),
  })
}

pub fn put(page:usize, mut surface:ReadbackSurface, pinned:bool, reads:u32){
  let bytes = surface.byte_size();
  let entry = Entry::new(surface, bytes, reads, pinned);
  charge(&entry);
  SURFACES.with_borrow_mut(|surfaces| surfaces.insert(page, entry));
}

pub fn worst() -> Option<(usize, Rank)>{
  SURFACES.with_borrow(worst_in)
}

fn retain(mut keep:impl FnMut(usize, &Entry<ReadbackSurface>) -> bool){
  SURFACES.with_borrow_mut(|surfaces|{
    let reclaimed:u64 = surfaces
      .extract_if(|&page, entry| !keep(page, entry))
      .map(|(_, entry)|{
        credit(&entry); // every removal is credited, but only CPU memory is reclaimable
        match entry.value.is_gpu_backed(){ true => 0, false => entry.bytes }
      })
      .sum();

    if reclaimed > 0{
      mem::glibc::mark_reclaimable(reclaimed as usize)
    }
  })
}

pub fn discard(page:usize){
  retain(|key, _| key != page)
}

pub fn expire(now:Instant){
  retain(|_, entry| !entry.expired(now))
}

pub fn clear(){
  retain(|_, _| false)
}

// pinned surfaces don't count toward the budget but are still live (until the context is retired)
pub fn holds_live_surfaces() -> bool{
  SURFACES.with_borrow(|surfaces| surfaces.values().any(|entry| entry.bytes > 0))
}

#[cfg(test)]
pub(super) fn seed(page:usize, bytes:u64, pinned:bool, reads:u32){
  let entry = Entry::new(ReadbackSurface::default(), bytes, reads, pinned);
  charge(&entry);
  SURFACES.with_borrow_mut(|surfaces| surfaces.insert(page, entry));
}
