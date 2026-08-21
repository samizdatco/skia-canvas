use std::fs;
use std::path::Path as FilePath;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use rayon::prelude::*;
use neon::prelude::*;
use skia_safe::{
  svg::{self, canvas::Flags},
  image::BitDepth, images, pdf, BlendMode,
  canvas::SaveLayerRec,
  Canvas as SkCanvas, ClipOp, Color4f, ColorSpace, ColorType, AlphaType, Document, Paint,
  Image as SkImage, ImageInfo, Matrix, Path, Picture, PictureRecorder, Point, Rect, IRect, Size, ISize,
  SurfaceProps, SurfacePropsFlags, PixelGeometry, SamplingOptions, jpeg_encoder, png_encoder, webp_encoder
};
use skia_safe::sampling_options::{FilterMode, MipmapMode};
use little_exif::{metadata::Metadata, exif_tag::ExifTag, filetype::FileExtension};
use crc::{Crc, CRC_32_ISO_HDLC};
const CRC32: Crc<u32> = Crc::<u32>::new(&CRC_32_ISO_HDLC);

use crate::canvas::BoxedCanvas;
use crate::context::BoxedContext2D;
use crate::gfx::RenderingEngine;
use crate::gfx::cache::{may_snapshot, Cache};
use crate::mem;

// the page's content is a list of layers which are either normal drawing ops or another page referenced by
// a drawCanvas()/drawImage() call, deferred so each backend (SVG/PDF/bitmap) can composite it appropriately
#[derive(Debug, Clone)]
pub enum Layer{
  Ops(Picture), // local drawing ops (flattened into a saveLayer)
  Page(Box<PageRef>), // another page, deferred until rendering
}

// a reference to another page, along with the geometry placing it in the destination
#[derive(Debug, Clone)]
pub struct PageRef{
  pub page: Page,         // the source, snapshotted at the moment it was drawn
  pub bounds: Rect,       // its frame-rect in the destination, with the draw-time CTM applied
  pub clip: Option<Path>, // set only when the CTM rotates/skews or a clip is live — i.e. when
                          // `bounds` alone can't describe the region (see `page_region`)
  pub matrix: Matrix,     // the CTM at draw-time (plus placement/scaling from the draw call's coords/dims)
}

impl PageRef{
  // transform `bounds` to the space the layers are being replayed into (e.g., a window `fit` transform
  // or the accumulated matrix of a nested page)
  fn bounds_in(&self, matrix:Option<&Matrix>) -> Rect{
    matrix.map(|m| m.map_rect(self.bounds).0).unwrap_or(self.bounds)
  }

  // narrow a canvas to the region (bounds + clip) this reference occupies
  fn clip_to(&self, canvas:&SkCanvas, matrix:Option<&Matrix>){
    match &self.clip{
      Some(clip) => match matrix{
        Some(matrix) => { canvas.clip_path(&clip.with_transform(matrix), ClipOp::Intersect, true); },
        None => { canvas.clip_path(clip, ClipOp::Intersect, true); },
      },
      None => { canvas.clip_rect(self.bounds_in(matrix), ClipOp::Intersect, true); },
    }
  }

  // open a transparency layer covering this reference's bounding box (the layer equivalent to `resolve()`)
  fn begin_layer(&self, canvas:&SkCanvas, matrix:Option<&Matrix>, paint:&Paint, space:Option<&ColorSpace>){
    let bounds = self.bounds_in(matrix);
    let rec = SaveLayerRec::default().bounds(&bounds).paint(paint);
    canvas.save_layer(&match space{
      Some(space) => rec.color_space(space),
      None => rec, // default to destination's color space if not overridden
    });
  }
}

// each export backend needs a different strategy for handling page refs & transparency groups
#[derive(Clone, Copy)]
pub enum Replay<'a>{
  Raster(Cache<'a>), // bitmap: resolve page refs to cached rasters where applicable (see `resolve`)
  Vector,            // pdf: replay page refs inside transparency layers, keeping their content as geometry
  Geometry,          // svg: no transparency layers at all (Skia's SVG support lacks them)
}

impl Replay<'_>{
  fn isolates(&self) -> bool{ !matches!(self, Replay::Geometry) }
}

// which version of a page's content a raster holds: its identity, bounds revision, & layer count.
// ordered rather than merely comparable — see `extends`
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct PageVersion{
  pub id: usize,    // the source page.id (reset whenever canvas content is fully cleared)
  pub epoch: u32,   // incremented when a non-destructive (window fit) resize occurs
  pub depth: usize, // number of layers incorporated into this raster
}

impl PageVersion{
  // whether the raster with this version can be the baseline for appending additional drawing
  pub fn extends(&self, now:&PageVersion) -> bool{
    self.id == now.id && self.epoch == now.epoch && self.depth <= now.depth
  }
}

// a one-shot latch shared by a recorder and every `Page` it emits. if any Page creates a
// texture-backed cache image while rendering, they can register the parent PageRecorder
// to post an eviction request to the render_thread when it is eventually dropped
#[derive(Debug, Clone, Default)]
struct PostedEviction(Arc<AtomicBool>);

impl PostedEviction{
  fn request(&self){
    self.0.store(true, Ordering::Relaxed)
  }

  // read and clear: disposal posts exactly one eviction, and a recorder is only released once
  fn claim(&self) -> bool{
    self.0.swap(false, Ordering::Relaxed)
  }
}

//
// Deferred canvas (records drawing commands for later replay on an output surface)
//

pub struct PageRecorder{
  current: Option<PictureRecorder>,
  layers: Vec<Layer>,
  bounds: Rect,
  matrix: Matrix,
  clip: Option<Path>,
  color_space: ColorSpace,
  changed: bool, // whether the recorder contains new ops not yet written to a Layer
  disposed: bool, // prevent additional drawing after PictureRecorder has been dropped
  eviction: PostedEviction, // whether cached rasters need to be dropped on the render thread
  approx_ops: usize, // draw ops recorded into `current` since the last get_page() flush (see release())
  footprint: mem::v8::Footprint, // report the retained display-list size to V8's GC accounting
  last_image: Option<(PageVersion, SkImage)>, // the most recent get_image() result, reused while the version holds
  dependent_ops: bool, // contains (non-reference) layers that use a clear, blit, or non-SrcOver blend
  has_pages: bool, // contains page-reference layers (added via `drawCanvas`/`drawImage`)
  id: usize, // generation id (incremented when the canvas is fully cleared)
  epoch: u32, // bounds id (incremented by a non-destructive window-fit resize)
}

impl PageRecorder{
  pub fn new(bounds:Rect, color_space:ColorSpace) -> Self {
    // recorder ids climb from 1 while `Page::picture_id`s descend from usize::MAX
    static COUNTER:AtomicUsize = AtomicUsize::new(1);
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);

    PageRecorder{
      current:None, layers:vec![], changed:false, disposed:false,
      matrix:Matrix::default(), clip:None, bounds, id, epoch:0, color_space,
      eviction:PostedEviction::default(),
      approx_ops:0,
      footprint:mem::v8::Footprint::default(),
      last_image:None,
      dependent_ops:false,
      has_pages:false,
    }
  }

  // record that the page will need an isolation group if referenced by another page
  pub fn mark_dependent(&mut self){
    self.dependent_ops = true;
  }

  // finish current recording and append as a layer
  pub fn flush(&mut self){
    if !self.changed { return } // no-op if nothing new to add

    // store layer as a drawable (so copies are deduplicated) wrapped in a picture (so it can be sent to other threads)
    let layer = self.current.as_mut().and_then(|rec| rec.finish_recording_as_drawable());
    if let Some(mut drawable) = layer{
      let mut wrapper = PictureRecorder::new();
      wrapper.begin_recording(self.bounds, true).draw_drawable(&mut drawable, None);
      if let Some(pict) = wrapper.finish_recording_as_picture(None){
        self.footprint.grow(pict.approximate_bytes_used() as i64); // update v8's accounting
        self.layers.push(Layer::Ops(pict));
      }
      self.approx_ops = 0;
    }

    if let Some(rec) = self.current.as_mut(){
      rec.begin_recording(self.bounds, true);
    }
    self.changed = false; // recorder is clean
    self.restore();
  }

  // append another page by reference
  pub fn push_page(&mut self, page_ref:PageRef){
    if self.disposed { return }
    self.flush();
    self.has_pages = true;
    self.layers.push(Layer::Page(Box::new(page_ref)));
  }

  pub fn append<F>(&mut self, f:F)
    where F:FnOnce(&SkCanvas)
  {
    if self.disposed{
      return // post-dispose draws must be ignored
    }else if self.current.is_none() {
      // allocate lazily on first draw op
      let mut rec = PictureRecorder::new();
      rec.begin_recording(self.bounds, true);
      self.current = Some(rec);
      self.restore();
    }

    if let Some(canvas) = self.current.as_mut().and_then(|rec| rec.recording_canvas()) {
      f(canvas);
      self.changed = true;
      self.approx_ops += 1;
    }
  }

  pub fn color_space(&self) -> ColorSpace{
    self.color_space.clone()
  }

  pub fn set_bounds(&mut self, bounds:Rect){
    *self = PageRecorder::new(bounds, self.color_space.clone());
  }

  pub fn update_bounds(&mut self, bounds:Rect){
    if bounds != self.bounds{
      self.bounds = bounds; // non-destructively update the size (the id and its layers survive)
      self.epoch += 1; // invalidate any rasters derived from the old bounds
    }
  }

  pub fn set_matrix(&mut self, matrix:Matrix){
    self.matrix = matrix;
    if let Some(canvas) = self.current.as_mut().and_then(|rec| rec.recording_canvas()) {
      canvas.set_matrix(&matrix.into());
    }
  }

  pub fn set_clip(&mut self, clip:&Option<Path>){
    self.clip = clip.clone();
    self.restore();
  }

  pub fn restore(&mut self){
    if let Some(canvas) = self.current.as_mut().and_then(|rec| rec.recording_canvas()) {
      canvas.restore_to_count(1);
      canvas.save();
      if let Some(clip) = &self.clip{
        canvas.clip_path(clip, ClipOp::Intersect, true /* antialias */);
      }
      canvas.set_matrix(&self.matrix.into());
    }
  }

  pub fn write_pixels(&mut self, dst_buffer:&mut [u8], dst_info:&ImageInfo, crop:IRect, opts:ExportOptions, engine:RenderingEngine, read_frequently:bool) -> Result<(), String>{
    // dst_buffer must be zero-filled since regions of the crop outside the canvas bounds won't be updated
    if !self.bounds.intersects(Rect::from_irect(crop)){
      return Ok(())
    }

    let page = self.get_page();
    match engine{
      // use the render-thread to rasterize (using the cached surface for this page)
      // then copy the pixels into the js-owned buffer
      RenderingEngine::GPU => {
        // no residency test needed: a GPU readback always rasterizes on the render thread
        self.eviction.request();
        let dst_info = dst_info.clone();
        let pixels = engine.render(move |cache|{
          cache.readback(page.id, read_frequently, |surface|{
            surface.update(&page, &opts, &engine, cache);
            let mut pixels = vec![0u8; dst_info.compute_min_byte_size()];
            match surface.copy_pixels(&dst_info, crop, &mut pixels){
              true => Ok(pixels),
              false => Err(format!("Could not get image data (format: {:?})", dst_info.color_type()))
            }
          })
        })?;
        dst_buffer.copy_from_slice(&pixels);
        Ok(())
      }

      RenderingEngine::CPU => {
        let cache = Cache::shared();
        cache.readback(page.id, read_frequently, |surface|{
          surface.update(&page, &opts, &engine, cache);
          match surface.copy_pixels(dst_info, crop, dst_buffer){
            true => Ok(()),
            false => Err(format!("Could not get image data (format: {:?})", dst_info.color_type()))
          }
        })
      }
    }
  }

  pub fn get_page(&mut self) -> Page{
    self.flush();

    Page{
      layers: self.layers.clone(),
      bounds: self.bounds,
      id: self.id,
      epoch: self.epoch,
      color_space: self.color_space.clone(),
      dependent_ops: self.dependent_ops,
      has_pages: self.has_pages,
      eviction: self.eviction.clone(),
    }
  }

  pub fn get_image(&mut self) -> Option<SkImage>{
    self.flush(); // move any uncommitted drawing ops to a layer

    // reuse the last image unless the canvas has been drawn into or resized since it was made
    let version = PageVersion{ id:self.id, epoch:self.epoch, depth:self.layers.len() };
    if let Some((cached, image)) = &self.last_image{
      if *cached == version{ return Some(image.clone()) }
    }

    let size = self.bounds.size().to_floor();
    let image = self
      .get_page()
      .to_picture(None)
      .and_then(|pict| {
        // rasterize using *extended-range* sRGB (via F16 colors) so it can later be converted into
        // whatever colorSpace drawImage(canvas) is using
        images::deferred_from_picture(
          pict, size, None, None, BitDepth::F16, Some(ColorSpace::new_srgb()), None
        )
      })?;

    self.last_image = Some((version, image.clone()));
    Some(image)
  }
}

impl PageRecorder{
  // synchronously release all internal Skia state (rather than waiting until for the next event
  // loop tick gets around to calling Drop)
  pub fn release(&mut self){
    if self.disposed { return; }
    self.disposed = true;

    // track a rough estimate of memory use to calibrate malloc_trim calls
    const BASE: usize = 16 * 1024; // size of an empty recorder
    const APPROX_OP_BYTES: usize = 128; // based on a 2500-arc recording ≈ 300k
    let estimated_bytes: usize = BASE
      + self.layers.iter().filter_map(|l| match l{
          Layer::Ops(p) => Some(p.approximate_bytes_used()), _ => None
        }).sum::<usize>()
      + self.approx_ops * APPROX_OP_BYTES;

    self.footprint.clear(); // credit the display-list charge back to V8
    self.current = None;
    self.last_image = None; // drop before the layers (since it references them)
    self.layers.clear();

    let id = self.id;
    let cache = Cache::shared();
    cache.evict(id); // drop the CPU rasters + this thread's readback surface
    cache.sweep(); // opportunistically sweep the rest of the CPU cache

    if self.eviction.claim(){
      // any textures this page left on the render thread can only be dropped *by* that thread
      crate::gfx::render_soon(move |cache| cache.evict(id));
    }

    mem::glibc::mark_reclaimable(estimated_bytes);
  }
}

impl Drop for PageRecorder{
  fn drop(&mut self) {
    self.release();
  }
}


//
// Image generator for a single drawing context
//

#[derive(Debug, Clone)]
pub struct Page{
  pub id: usize,
  pub bounds: Rect,
  layers: Vec<Layer>,
  epoch: u32, // bounds revision under this id
  pub color_space: ColorSpace, // inherited from the context that recorded the page
  pub dependent_ops: bool, // contains ops that clear, blit, or use a non-SrcOver blend
  pub has_pages: bool, // contains other pages included by reference
  eviction: PostedEviction, // whether any of the page's cached rasters are textures on the render_thread
}

impl PartialEq for Page {
  fn eq(&self, other: &Self) -> bool {
    self.version() == other.version()
  }
}

impl Default for Page {
  fn default() -> Self {
    Self{ id:0, bounds: skia_safe::Rect::new_empty(), layers:vec![], epoch:0,
          color_space: ColorSpace::new_srgb(), dependent_ops:false, has_pages:false,
          eviction: PostedEviction::default() }
  }
}

impl Page{
  // derive a page.id for (vector) Images that have been wrapped in a Page
  pub fn picture_id(pict:&Picture) -> usize{
    usize::MAX - pict.unique_id() as usize // reuse Skia's monotonic unique IDs
  }

  // wrap a Picture in a single-layer Page so it can be cached just like canvas-based vector sources
  pub fn from_picture(pict:&Picture, size:Size) -> Self{
    Self{
      id: Self::picture_id(pict),
      bounds: Rect::from_size(size),
      layers: vec![Layer::Ops(pict.clone())],
      ..Default::default() // srgb, epoch 0, no dependent ops, no page refs
    }
  }

  pub fn depth(&self) -> usize{
    self.layers.len()
  }

  pub fn version(&self) -> PageVersion{
    PageVersion{ id: self.id, epoch: self.epoch, depth: self.layers.len() }
  }

  // flag that this page created texture-backed rasters that must be dropped on the render_thread
  pub fn evict_on_render_thread(&self){
    self.eviction.request()
  }

  pub fn scaled_dimensions(&self, density:f32) -> ISize{
    Size::new(self.bounds.width() * density, self.bounds.height() * density).to_floor()
  }

  // draw all or a slice of the page's layers into a canvas, optionally adding a matrix transform to each.
  // the `replay` mode selects how isolation groups are handled (for bitmaps vs PDF vs SVG)
  pub fn playback_from(&self, canvas:&SkCanvas, first:usize, matrix:Option<&Matrix>, replay:Replay){
    for layer in self.layers.iter().skip(first){
      match layer{
        Layer::Ops(pict) => { canvas.draw_picture(pict, matrix, None); },
        Layer::Page(page_ref) => {
          let total = match matrix{ Some(m) => Matrix::concat(m, &page_ref.matrix), None => page_ref.matrix };

          // use resolve() to pre-rasterize where possible (raster destination, unflipped, axis-aligned)
          let resolved = match replay{
            Replay::Raster(cache) => Self::resolve(cache, canvas, page_ref, &total),
            _ => None,
          };

          // clip before opening the layer, so the boundary encloses the *group* rather than each op
          // inside it (otherwise an erasing op will leave an un-erased fringe along the boundary)
          canvas.save();
          page_ref.clip_to(canvas, matrix);
          match resolved{
            Some((image, at)) => {
              // blit without applying the CTM; the raster is already at device scale
              canvas.save();
              canvas.reset_matrix();
              let sampling = SamplingOptions::new(FilterMode::Linear, MipmapMode::None);
              canvas.draw_image_with_sampling_options(&image, at, sampling, None);
              canvas.restore();
            }
            None => {
              // when drawing an sRGB canvas into display-p3, keep its colors clamped in sRGB
              let remap_gamut = match canvas.image_info().color_space(){
                Some(dst) if dst != page_ref.page.color_space && page_ref.page.color_space.is_srgb() =>
                  Some(page_ref.page.color_space.clone()),
                _ => None
              };

              // only use an isolation layer when the source requires it or there's a gamut remap
              match (replay.isolates() && page_ref.page.dependent_ops) || remap_gamut.is_some(){
                true => {
                  // use a non-null paint, so skia doesn't filter the transparency layer out as a no-op
                  page_ref.begin_layer(canvas, matrix, &Paint::default(), remap_gamut.as_ref());
                  page_ref.page.playback_from(canvas, 0, Some(&total), replay);
                  canvas.restore();
                }
                false => page_ref.page.playback_from(canvas, 0, Some(&total), replay)
              }
            }
          }
          canvas.restore();
        }
      }
    }
  }

  // rasterize (and cache) a referenced page at its final scale and sub-pixel position
  fn resolve(cache:Cache, canvas:&SkCanvas, page_ref:&PageRef, total:&Matrix) -> Option<(SkImage, Point)>{
    if canvas.image_info().color_type() == ColorType::Unknown{ return None } // confirm destination is actually a raster

    // a raster can only be useful for blitting if the placement is axis-aligned and un-flipped
    let device = Matrix::concat(&canvas.local_to_device_as_3x3(), total);
    if device.skew_x() != 0.0 || device.skew_y() != 0.0 || device.has_perspective(){ return None }
    let (sx, sy) = (device.scale_x(), device.scale_y());
    if !(sx.is_finite() && sy.is_finite() && sx > 0.0 && sy > 0.0){ return None }

    // split the position into a whole-pixel origin and sub-pixel phases (quantized to 16ths)
    let placed = device.map_rect(page_ref.page.bounds).0;
    let origin = Point::new(placed.left.floor(), placed.top.floor());
    let bucket = |v:f32| ((v * 16.0).round() as u64) & 0xff;
    let (px, py) = (bucket(placed.left - origin.x), bucket(placed.top - origin.y));
    let phase = Point::new(px as f32 / 16.0, py as f32 / 16.0);

    let size = page_ref.page.bounds.size();
    let (w, h) = ((size.width * sx + phase.x).ceil(), (size.height * sy + phase.y).ceil());
    if !(w >= 1.0 && h >= 1.0 && w <= 8192.0 && h <= 8192.0){ return None }

    // pack the scale and sub-pixel phase into the cache's placement key, in disjoint bit fields so
    // the two can't alias: 5 bits per phase axis (`bucket` above caps each at 16) and 27 per scale,
    // quantized to 1/64 — which saturates well past any scale the 8192² cap admits
    let q = |v:f32| ((v * 64.0).round() as u64).min((1 << 27) - 1);
    let key = (px << 59) | (py << 54) | (q(sx) << 27) | q(sy);
    let cost = (w as u64) * (h as u64) * 4;
    let dims = ISize::new(w as i32, h as i32);
    // the callback runs only when there is no usable raster yet; a reusable one comes back without it, and
    // so does a full cache (landing here as `None` and sending the caller off to replay geometry)
    let image = cache.page_raster(&page_ref.page, key, dims, cost, |base|{
      let info = ImageInfo::new_n32_premul(dims, Some(page_ref.page.color_space.clone()));
      let mut surface = canvas.new_surface(&info, None)?; // inherit the canvas's device
      {
        let target = surface.canvas();
        let first = match base{
          // if a raster of an earlier slice exists, use it as a base and add just the new layers
          Some((image, covered)) => { target.draw_image(image, (0, 0), None); covered },
          None => 0,
        };
        target.translate((phase.x, phase.y));
        target.scale((sx, sy));
        page_ref.page.playback_from(target, first, None, Replay::Raster(cache));
      }
      Some(surface.image_snapshot())
    })?;

    Some((image, origin))
  }

  // flatten the page to a matted Picture with page refs handled appropriately for the `replay` destination
  fn flatten(&self, matte:Option<Color4f>, replay:Replay) -> Option<Picture> {
    let mut recorder = PictureRecorder::new();
    let output = recorder.begin_recording(self.bounds, true);
    if let Some(color) = matte{ output.clear(color); }

    // create a transparency layer if the content needs it (so it doesn't modify the matte)
    match replay.isolates() && self.dependent_ops{
      true => {
        output.save_layer(&SaveLayerRec::default().bounds(&self.bounds).paint(&Paint::default()));
        self.playback_from(output, 0, None, replay);
        output.restore();
      }
      false => self.playback_from(output, 0, None, replay)
    }

    recorder.finish_recording_as_picture(None)
  }

  pub fn to_picture(&self, matte:Option<Color4f>) -> Option<Picture> {
    self.flatten(matte, Replay::Vector)
  }

  pub fn encoded_as(&self, options:ExportOptions, engine:RenderingEngine) -> Result<Vec<u8>, String> {
    if self.bounds.is_empty(){
      return Err("Width and height must be non-zero to generate an image".to_string())
    }

    let ExportOptions{ ref format, quality, density, matte, .. } = options;
    let size = self.bounds.size();
    let img_quality = ((quality*100.0) as u32).clamp(0, 100);

    match format.as_str(){
      "pdf" => {
        let mut pdf_bytes = Vec::new();
        let metadata = pdf_metadata(quality, density);
        self.append_to(pdf_document(&mut pdf_bytes, &metadata), matte)?.close();
        Ok(pdf_bytes)
      }

      "svg" => {
        let canvas = svg::Canvas::new(Rect::from_size(size), options.svg_flags());
        let picture = self.flatten(matte, Replay::Geometry).ok_or("Could not generate an image")?;
        canvas.draw_picture(&picture, None, None);
        Ok(canvas.end().as_bytes().to_vec())
      }

      // handle bitmap formats using (potentially gpu-backed) rasterizer
      _ => {
        // rasterize on the shared render thread (or inline when CPU-based), returning a
        // non-texture-backed snapshot; encoding happens back on the calling thread so
        // concurrent exports can still parallelize the compression work
        let page = self.clone();
        let opts = options.clone();

        enum Rendered{ Encodable(SkImage), Raw(Vec<u8>) }
        let rendered = engine.render(move |cache| {
          let color_space = page.color_space.clone(); // colorSpace currently can't be overridden for exports
          let img_info = ImageInfo::new_n32_premul(page.scaled_dimensions(opts.density), Some(color_space.clone()));
          let cached = may_snapshot(page.depth()); // shallow pages re-render cheaply; don't spend a slot
          let mut surface = engine.make_surface(&img_info, &opts, !cached)?;
          let canvas = surface.canvas();

          // check the cache for a bitmap to provide a base for layered drawing *or* a signal that one had been
          // previously requested (i.e., that this is a second pass so the bitmap should be kept this time)
          let (first, keep) = match cached{
            false => (0, false),
            true => cache.export(&page, &opts, &color_space, |snap, config|{
              let first = match snap.accepts(&page, config){
                true => { canvas.draw_image(snap.image.as_ref().unwrap(), (0,0), None); snap.version.depth }
                false => 0
              };
              (first, snap.recur())
            })
          };


          // replay the layers the snapshot doesn't already cover
          canvas.set_matrix(&Matrix::scale((opts.density, opts.density)).into());
          page.playback_from(canvas, first, None, Replay::Raster(cache));

          // cache the raster as the backdrop for the next export of the same page
          if keep{
            if let Some(image) = surface.image_snapshot_with_bounds(img_info.bounds()){
              let bytes = img_info.compute_min_byte_size() as u64;
              cache.export(&page, &opts, &color_space,
                |snap, config| snap.store(Some(image), &page, config, bytes));
            }
          }

          // draw the matte underneath as a final pass so the page's content has a transparent background
          // to work with and the cached bitmaps don't have a baked-in matte
          if let Some(color) = opts.matte{
            surface.canvas().draw_color(color, BlendMode::DstOver);
          }

          // extract the results
          let image = surface.make_temporary_image()
            .or_else(|| surface.image_snapshot_with_bounds(img_info.bounds()))
            .ok_or("Could not read canvas contents (GPU context lost)".to_string())?;

          match opts.format.as_str() {
            "raw" => {
              // return a Rendered::Raw buffer of pixels converted to destination color type
              let dst_info = ImageInfo::new(page.scaled_dimensions(opts.density), opts.color_type, AlphaType::Unpremul, Some(color_space.clone()));
              let mut buffer: Vec<u8> = vec![0; dst_info.compute_min_byte_size()];
              match surface.read_pixels(&dst_info, &mut buffer, dst_info.min_row_bytes(), (0,0)){
                true => Ok(Rendered::Raw(buffer)),
                false => Err(format!("Could not encode as raw ({:?})", opts.color_type))
              }
            }
            _ => {
              // return a Rendered::Encodable image that's been moved off the GPU
              match image.is_texture_backed(){
                true => image.make_non_texture_image(&mut surface.direct_context())
                  .ok_or("Could not read canvas contents (GPU context lost)".to_string()),
                false => Ok(image)
              }.map(Rendered::Encodable)
            }
          }
        })?;

        // "raw" is already-converted pixel bytes; everything else is a raster image to compress
        let image = match rendered{
          Rendered::Raw(buffer) => return Ok(buffer),
          Rendered::Encodable(image) => image,
        };

        // handle image encoding (image is always raster-backed, so no gpu context is needed)
        let context: Option<&mut skia_safe::gpu::DirectContext> = None;
        match format.as_str(){
          "raw" => unreachable!("raw handled on the render thread"),

          "jpg" | "jpeg" => {
            let jpg_opts = jpeg_encoder::Options {
                quality: img_quality,
                downsample: match options.jpeg_downsample{
                  true => jpeg_encoder::Downsample::BothDirections,
                  false => jpeg_encoder::Downsample::No,
                },
                ..jpeg_encoder::Options::default()
            };

            jpeg_encoder::encode_image(context, &image, &jpg_opts).map(|data|{
              let mut bytes = data.as_bytes().to_vec();
              let [l, r] = (72 * density as u16).to_be_bytes();
              bytes.splice(13..18, [1, l, r, l, r].iter().cloned());
              bytes
            })
          }

          "png" => {
            let mut png_opts = png_encoder::Options::default();
            png_opts.filter_flags = png_encoder::FilterFlag::NONE;
            png_opts.z_lib_level = match quality{
              // use `quality` to control zlib 'effort' (defaulting to 6)
              q if q < 0.925 => ((q / 0.92 * 6.0).round() as i32).clamp(1, 6),
              q => (7.0 + (q - 0.925) / 0.075 * 2.0).round() as i32
            };

            png_encoder::encode_image(context, &image, &png_opts).map(|data|{
              let mut bytes = data.as_bytes().to_vec();
              let mut digest = CRC32.digest();
              let [a, b, c, d] = ((72.0 * density * 39.3701) as u32).to_be_bytes();
              let phys = vec![
                b'p', b'H', b'Y', b's',
                a, b, c, d, // x-dpi
                a, b, c, d, // y-dpi
                1, // dots per meter
              ];
              digest.update(&phys);

              let length = 9u32.to_be_bytes().to_vec();
              let checksum = digest.finalize().to_be_bytes().to_vec();
              bytes.splice(33..33, [length, phys, checksum].concat());
              bytes
            })
          }

          "webp" => {
            let mut webp_opts = webp_encoder::Options::default();
            if img_quality == 100 {
                webp_opts.compression = webp_encoder::Compression::Lossless;
                webp_opts.quality = 75.0;
            } else {
                webp_opts.compression = webp_encoder::Compression::Lossy;
                webp_opts.quality = img_quality as _;
            }

            webp_encoder::encode_image(context, &image, &webp_opts).map(|data|{
              let mut bytes = data.as_bytes().to_vec();

              // toggle EXIF flag in VP8X chunk
              bytes[20] |= 1 << 3;

              // append EXIF chunk with DPI
              let dpi = (72.0 * density) as f64;
              let mut exif = Metadata::new();
              exif.set_tag( ExifTag::XResolution(vec![dpi.into()]) );
              exif.set_tag( ExifTag::YResolution(vec![dpi.into()]) );
              if let Ok(mut exif_bytes) = exif.as_u8_vec(FileExtension::WEBP){
                bytes.append(&mut exif_bytes);
              }

              // update file-length field in RIFF header
              let file_size = ((bytes.len() - 8) as u32).to_le_bytes();
              bytes.splice(4..8, file_size.iter().cloned());

              bytes
            })
          }
          _ => return Err(format!("Unsupported file format {}", format))
        }.ok_or(format!("Could not encode as {}", format))
      }
    }
  }

  pub fn write(&self, filename: &str, options:ExportOptions, engine:RenderingEngine) -> Result<(), String> {
    let path = FilePath::new(&filename);
    let data = self.encoded_as(options, engine)?;
    fs::write(path, data).map_err(|why|
      format!("{}: \"{}\"", why, path.display())
    )
  }

  fn append_to<'a>(&self, doc:Document<'a>, matte:Option<Color4f>) -> Result<Document<'a>, String>{
    if !self.bounds.is_empty(){
      let mut doc = doc.begin_page(self.bounds.size(), None);
      let canvas = doc.canvas();
      if let Some(picture) = self.to_picture(matte){
        canvas.draw_picture(&picture, None, None);
      }
      Ok(doc.end_page())
    }else{
      Err("Width and height must be non-zero to generate a PDF page".to_string())
    }
  }
}

//
// Container for a canvas's entire stack of page contexts
//

pub struct PageSequence{
  pub pages: Vec<Page>,
  pub engine: RenderingEngine
}

impl PageSequence{
  pub fn from(pages:Vec<Page>, engine:RenderingEngine) -> Self{
    PageSequence { pages, engine }
  }

  pub fn first(&self) -> &Page {
    &self.pages[0]
  }

  pub fn len(&self) -> usize{
    self.pages.len()
  }

  pub fn as_pdf(&self, options:ExportOptions) -> Result<Vec<u8>, String>{
    let ExportOptions{ quality, density, matte, .. } = options;
    let mut pdf_bytes = Vec::new();
    let metadata = pdf_metadata(quality, density);
    self.pages
      .iter()
      .try_fold(pdf_document(&mut pdf_bytes, &metadata), |doc, page| page.append_to(doc, matte))
      .map(|doc| doc.close())?;
    Ok(pdf_bytes)
  }

  pub fn write_image(&self, pattern:&str, options:ExportOptions) -> Result<(), String>{
    self.first().write(pattern, options, self.engine)
  }

  #[allow(clippy::too_many_arguments)]
  pub fn write_sequence(&self, pattern:&str, padding:f32, options:ExportOptions) -> Result<(), String>{
    let padding = match padding as i32{
      -1 => (1.0 + (self.pages.len() as f32).log10().floor()) as usize,
      pad => pad as usize
    };

    let engine = self.engine;
    self.pages
      .clone() // page refs' clip paths are not Sync so need a clone (just a refcount) to jump threads
      .into_par_iter()
      .enumerate()
      .try_for_each(|(pp, page)|{
        let folio = format!("{:0width$}", pp+1, width=padding);
        let filename = pattern.replace("{}", folio.as_str());
        page.write(&filename, options.clone(), engine)
      })
  }

  pub fn write_pdf(&self, path:&str, options:ExportOptions) -> Result<(), String>{
    let path = FilePath::new(&path);
    match self.as_pdf(options){
      Ok(document) => fs::write(path, document).map_err(|why|
        format!("{}: \"{}\"", why, path.display())
      ),
      Err(msg) => Err(msg)
    }
  }
}

//
// Helpers
//

pub fn pages_arg(cx: &mut FunctionContext, idx:usize, canvas:&BoxedCanvas) -> NeonResult<PageSequence> {
  Cache::shared().sweep(); // opportunistic cache sweep on export
  let engine = canvas.borrow_mut().engine();
  let pages = cx.argument::<JsArray>(idx)?
      .to_vec(cx)?
      .iter()
      .map(|obj| obj.downcast::<BoxedContext2D, _>(cx))
      .filter( |ctx| ctx.is_ok() )
      .map(|obj| obj.unwrap().borrow().get_page())
      .collect();
  Ok(PageSequence::from(pages, engine))
}

fn pdf_metadata(quality:f32, density:f32) -> pdf::Metadata<'static>{
  pdf::Metadata {
    producer: "Skia Canvas <https://skia-canvas.org>".to_string(),
    encoding_quality: Some((quality*100.0) as i32),
    raster_dpi: Some(density * 72.0),
    ..Default::default()
  }
}

// the metadata must now outlive the Document, so it needs a separate constructor
fn pdf_document<'a>(buffer:&'a mut impl std::io::Write, metadata:&'a pdf::Metadata<'a>) -> Document<'a>{
  pdf::new_document(buffer, Some(metadata))
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExportOptions{
  pub format: String,
  pub quality: f32,
  pub density: f32,
  pub outline: bool,
  pub matte: Option<Color4f>,
  pub msaa: Option<usize>,
  pub color_type: ColorType,
  pub color_space: Option<ColorSpace>, // when unset, the Page being rendered supplies its own
  pub jpeg_downsample: bool,
  pub text_contrast: f32,
  pub text_gamma: f32,
}

impl Default for ExportOptions{
  fn default() -> Self {
    Self{
      format:"raw".to_string(), quality:0.92, density:1.0, matte:None,
      jpeg_downsample:false, text_contrast:0.0, text_gamma:1.4, msaa:None,
      color_type:ColorType::RGBA8888, color_space:None, outline:true,
    }
  }
}

impl ExportOptions{
  pub fn surface_props(&self) -> SurfaceProps{
    SurfaceProps::new_with_text_properties(
      SurfacePropsFlags::default(),
      PixelGeometry::Unknown,
      self.text_contrast,
      self.text_gamma,
    )
  }

  pub fn svg_flags(&self) -> Option<skia_safe::svg::canvas::Flags>{
    match self.outline{
      true => Some(Flags::CONVERT_TEXT_TO_PATHS),
      _ => None
    }
  }

  #[cfg(any(feature = "metal", feature = "vulkan"))] // GPU-only: MSAA sample selection for gpu surfaces
  pub fn msaa_from(&self, valid_msaa:&Vec<usize>) -> Result<usize, String>{
    let samples = self.msaa.unwrap_or(0); // default to shader-based AA
    match valid_msaa.contains(&samples){
      true => Ok(samples),
      false => Err(format!("{}x MSAA not supported by GPU (options: {:?})", samples, valid_msaa))
    }
  }
}
