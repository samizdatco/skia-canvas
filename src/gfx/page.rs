use std::fs;
use std::path::Path as FilePath;
use std::sync::atomic::{AtomicUsize, Ordering};
use rayon::prelude::*;
use neon::prelude::*;
use skia_safe::{
  svg::{self, canvas::Flags},
  image::BitDepth, images, pdf,
  Canvas as SkCanvas, ClipOp, Color, Color4f, ColorSpace, ColorType, AlphaType, Document, Surface,
  Image as SkImage, ImageInfo, Matrix, Path, Picture, PictureRecorder, Rect, IRect, Size, ISize,
  SurfaceProps, SurfacePropsFlags, PixelGeometry, jpeg_encoder, png_encoder, webp_encoder
};
use little_exif::{metadata::Metadata, exif_tag::ExifTag, filetype::FileExtension};
use crc::{Crc, CRC_32_ISO_HDLC};
const CRC32: Crc<u32> = Crc::<u32>::new(&CRC_32_ISO_HDLC);

use crate::canvas::BoxedCanvas;
use crate::context::BoxedContext2D;
use crate::gfx::RenderingEngine;
use crate::gfx::cache::SurfaceCache;
use crate::mem;

//
// Deferred canvas (records drawing commands for later replay on an output surface)
//

pub struct PageRecorder{
  current: Option<PictureRecorder>,
  layers: Vec<Picture>,
  bounds: Rect,
  matrix: Matrix,
  clip: Option<Path>,
  surface: RecordingSurface,
  changed: bool,
  disposed: bool, // flag that drawing ops should be ignored after PictureRecorder has been dropped
  id: usize,
  has_gpu_surface: bool, // flag that drops need to happen on render thread
  approx_ops: usize, // draw ops recorded into `current` since the last get_page() flush (see release())
}

impl PageRecorder{
  pub fn new(bounds:Rect) -> Self {
    static COUNTER:AtomicUsize = AtomicUsize::new(1);
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);

    PageRecorder{
      current:None, layers:vec![], changed:false, disposed:false,
      matrix:Matrix::default(), clip:None, bounds, id,
      surface:RecordingSurface::default(), has_gpu_surface:false,
      approx_ops:0,
    }
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

  pub fn set_bounds(&mut self, bounds:Rect){
    *self = PageRecorder::new(bounds);
  }

  pub fn update_bounds(&mut self, bounds:Rect){
    self.bounds = bounds; // non-destructively update the size
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

  pub fn write_pixels(&mut self, dst_buffer:&mut [u8], dst_info:&ImageInfo, crop:IRect, opts:ExportOptions, engine:RenderingEngine) -> Result<(), String>{
    // dst_buffer must be zero-filled since regions of the crop outside the canvas bounds won't be updated
    if !self.bounds.intersects(Rect::from_irect(crop)){
      return Ok(())
    }

    let page = self.get_page();
    match engine{
      // use the render-thread to rasterize (using the cached surface for this page)
      // then copy the pixels into the js-owned buffer
      RenderingEngine::GPU => {
        self.has_gpu_surface = true; // remember to free the gpu-backed export surface
        let dst_info = dst_info.clone();
        let pixels = engine.render(move ||{
          SurfaceCache::with_entry(page.id, |surface|{
            surface.update(&page, &opts, &engine);
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
        self.surface.update(&page, &opts, &engine);
        match self.surface.copy_pixels(dst_info, crop, dst_buffer){
          true => Ok(()),
          false => Err(format!("Could not get image data (format: {:?})", dst_info.color_type()))
        }
      }
    }
  }

  pub fn get_page(&mut self) -> Page{
    if self.changed {
      // store layer as a drawable (so copies are deduplicated) wrapped in a picture (so it can be sent to other threads)
      let layer = self.current.as_mut().and_then(|rec| rec.finish_recording_as_drawable());
      if let Some(mut drawable) = layer{
        let mut wrapper = PictureRecorder::new();
        wrapper.begin_recording(self.bounds, true).draw_drawable(&mut drawable, None);
        if let Some(pict) = wrapper.finish_recording_as_picture(None){
          self.layers.push(pict);
        }
        self.approx_ops = 0; // flushed content is now measurable via the layer picture itself
      }

      if let Some(rec) = self.current.as_mut(){
        rec.begin_recording(self.bounds, true);
      }
      self.changed = false; // recorder is clean
      self.restore();
    }

    Page{
      layers: self.layers.clone(),
      bounds: self.bounds,
      id: self.id,
    }
  }

  pub fn get_image(&mut self) -> Option<SkImage>{
    let size = self.bounds.size().to_floor();
    self
      .get_page()
      .get_picture(None)
      .and_then(|pict| {
        // rasterize using wide-gamut F16 colors so it can be converted into whatever colorSpace
        // drawImage(canvas) is using
        images::deferred_from_picture(
          pict, size, None, None, BitDepth::F16, Some(ColorSpace::new_srgb()), None
        )
      })
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
      + self.layers.iter().map(|pict| pict.approximate_bytes_used()).sum::<usize>()
      + self.approx_ops * APPROX_OP_BYTES;

    self.current = None;
    self.layers.clear();
    self.surface = RecordingSurface::default();

    // if an export has used a gpu surface, its RecordingSurface lives on the render thread
    // (in the SurfaceCache) and can only be dropped there
    let id = self.id;
    if self.has_gpu_surface{
      crate::gfx::render_soon(move ||{
        SurfaceCache::evict(id);
      });
      self.has_gpu_surface = false;
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
// Persistent GPU/CPU surface for caching intermediate results of getImageData()
//

pub struct RecordingSurface{
  surface: Option<Surface>,
  footprint: mem::v8::Footprint,
  depth: usize,
  matte: Option<Color4f>,
  msaa: Option<usize>,
  gpu: Option<bool>,
  color_space: ColorSpace,
  density: f32,
}

impl Default for RecordingSurface{
  fn default() -> Self {
    Self{surface:None, footprint:mem::v8::Footprint::default(), depth:0, matte:None, msaa:None, gpu:None, color_space:ColorSpace::new_srgb(), density:0.0}
  }
}

impl RecordingSurface{

  fn is_surface_stale(&mut self, page:&Page, opts:&ExportOptions, engine:&RenderingEngine) -> bool{
    let gpu_toggled = self.gpu != Some(matches!(engine, RenderingEngine::GPU));
    let page_size = page.scaled_dimensions(opts.density);
    let resized = self.surface.as_mut().map(|surface|{
      surface.image_info().dimensions() != page_size
    }).unwrap_or(true);

    gpu_toggled || resized
  }

  fn is_config_stale(&self, opts:&ExportOptions) -> bool{
    self.density != opts.density ||
    self.matte != opts.matte ||
    self.msaa != opts.msaa ||
    self.color_space != opts.color_space
  }

  pub fn update(&mut self, page:&Page, opts:&ExportOptions, engine:&RenderingEngine){
    // check for anything that would invalidate the previous contents
    let reconfigure = self.is_config_stale(&opts);
    let recreate = self.is_surface_stale(&page, &opts, &engine);

    // start from scratch if invalidated
    if reconfigure || recreate{
      self.gpu = Some(matches!(engine, RenderingEngine::GPU));
      self.color_space = opts.color_space.clone();
      self.density = opts.density;
      self.matte = opts.matte;
      self.msaa = opts.msaa;
      self.depth = 0;

      // only allocate a new surface if the dimensions (size * density) have changed or engine switched
      if recreate{
        let page_size = page.scaled_dimensions(opts.density);
        let img_info = ImageInfo::new_n32_premul(page_size, opts.color_space.clone());
        self.surface = engine.make_surface(&img_info, &opts).ok();

        let bytes = if self.surface.is_some(){ img_info.compute_min_byte_size() } else { 0 };
        self.footprint.set(bytes); // record the allocation size for v8
      }
    }

    if let Some(surface) = self.surface.as_mut(){
      let canvas = surface.canvas();

      // fill a fresh/recreated surface with the matte; a persistent surface keeps its prior contents
      // and just replays the layers added since the last update
      if self.depth==0 {
        canvas.clear(self.matte.unwrap_or(Color::TRANSPARENT.into()));
      }

      // only add new layers to surface
      canvas.scale((self.density, self.density));

      // draw newly added layers
      for pict in page.layers.iter().skip(self.depth){
        pict.playback(canvas);
      }
      self.depth = page.layers.len();
    }
  }

  pub fn copy_pixels(&mut self, dst_info: &ImageInfo, src: IRect, pixels: &mut [u8]) -> bool{
    self.surface.as_mut().map(|surface|{
      surface.read_pixels(dst_info, pixels, dst_info.min_row_bytes(), (src.x(), src.y()))
    }).unwrap_or(false)
  }
}


//
// Image generator for a single drawing context
//

#[derive(Debug, Clone)]
pub struct Page{
  pub id: usize,
  pub bounds: Rect,
  pub layers: Vec<Picture>,
}

impl PartialEq for Page {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id &&
    self.depth() == other.depth()
  }
}

impl Default for Page {
  fn default() -> Self {
    Self{ id:0, bounds: skia_safe::Rect::new_empty(), layers:vec![] }
  }
}

impl Page{
  pub fn depth(&self) -> usize{
    self.layers.len()
  }

  pub fn scaled_dimensions(&self, density:f32) -> ISize{
    Size::new(self.bounds.width() * density, self.bounds.height() * density).to_floor()
  }

  pub fn get_picture(&self, matte:Option<Color4f>) -> Option<Picture> {
    let mut compositor = PictureRecorder::new();
    let output = compositor.begin_recording(self.bounds, true);
    matte.map(|c| output.clear(c));
    self.layers.iter().for_each(|pict| pict.playback(output));
    compositor.finish_recording_as_picture(None)
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
        let mut document = pdf_document(&mut pdf_bytes, &metadata).begin_page(size, None);
        let canvas = document.canvas();
        let picture = self.get_picture(matte).ok_or("Could not generate an image")?;
        canvas.draw_picture(&picture, None, None);
        document.end_page().close();
        Ok(pdf_bytes)
      }

      "svg" => {
        let canvas = svg::Canvas::new(Rect::from_size(size), options.svg_flags());
        let picture = self.get_picture(matte).ok_or("Could not generate an image")?;
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
        let rendered = engine.render(move || {
          let img_info = ImageInfo::new_n32_premul(page.scaled_dimensions(opts.density), Some(opts.color_space.clone()));
          let mut surface = engine.make_surface(&img_info, &opts)?;
          let canvas = surface.canvas();

          // fill the canvas if a matte was requested
          if let Some(color) = opts.matte{
            canvas.clear(color);
          }

          // replay all recorded layers into the transient surface (freed at closure end)
          canvas.set_matrix(&Matrix::scale((opts.density, opts.density)).into());
          for pict in page.layers.iter(){
            pict.playback(canvas);
          }

          // extract the results (potentially texture-backed)
          let image = surface.make_temporary_image()
            .or_else(|| surface.image_snapshot_with_bounds(img_info.bounds()))
            .ok_or("Could not read canvas contents (GPU context lost)".to_string())?;

          match opts.format.as_str() {
            "raw" => {
              // return a Rendered::Raw buffer of pixels converted to destination color type
              let dst_info = ImageInfo::new(page.scaled_dimensions(opts.density), opts.color_type, AlphaType::Unpremul, Some(opts.color_space.clone()));
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
      if let Some(picture) = self.get_picture(matte){
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

    self.pages
      .par_iter()
      .enumerate()
      .try_for_each(|(pp, page)|{
        let folio = format!("{:0width$}", pp+1, width=padding);
        let filename = pattern.replace("{}", folio.as_str());
        page.write(&filename, options.clone(), self.engine)
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
  pub color_space: ColorSpace,
  pub jpeg_downsample: bool,
  pub text_contrast: f32,
  pub text_gamma: f32,
}

impl Default for ExportOptions{
  fn default() -> Self {
    Self{
      format:"raw".to_string(), quality:0.92, density:1.0, matte:None,
      jpeg_downsample:false, text_contrast:0.0, text_gamma:1.4, msaa:None,
      color_type:ColorType::RGBA8888, color_space:ColorSpace::new_srgb(), outline:true,
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
