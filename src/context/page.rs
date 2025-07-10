use std::fs;
use std::path::Path as FilePath;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};
use rayon::prelude::*;
use neon::prelude::*;
use skia_safe::{
  svg::{self, canvas::Flags},
  image::{BitDepth, CachingHint}, images, pdf,
  Canvas as SkCanvas, ClipOp, Color, ColorSpace, ColorType, AlphaType, Document, Surface,
  Image as SkImage, ImageInfo, Matrix, Path, Picture, PictureRecorder, Rect, IRect, Size,
  SurfaceProps, SurfacePropsFlags, PixelGeometry, jpeg_encoder, png_encoder, webp_encoder
};
use dashmap::DashMap;
use little_exif::{metadata::Metadata, exif_tag::ExifTag, filetype::FileExtension};
use crc::{Crc, CRC_32_ISO_HDLC};
const CRC32: Crc<u32> = Crc::<u32>::new(&CRC_32_ISO_HDLC);

use crate::canvas::BoxedCanvas;
use crate::context::BoxedContext2D;
use crate::gpu::RenderingEngine;

static CACHE: OnceLock<Arc<DashMap<usize, PageCache>>> = OnceLock::new();

//
// Deferred canvas (records drawing commands for later replay on an output surface)
//

pub struct PageRecorder{
  current: PictureRecorder,
  layers: Vec<Picture>,
  bounds: Rect,
  matrix: Matrix,
  clip: Option<Path>,
  changed: bool,
  id: usize,
}

impl PageRecorder{
  pub fn new(bounds:Rect) -> Self {
    static COUNTER:AtomicUsize = AtomicUsize::new(1);
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    PageCache::add(id);

    let mut rec = PictureRecorder::new();
    rec.begin_recording(bounds, None);
    rec.recording_canvas().unwrap().save(); // start at depth 2

    PageRecorder{
      current:rec, layers:vec![], changed:false, matrix:Matrix::default(), clip:None, bounds, id
    }
  }

  pub fn append<F>(&mut self, f:F)
    where F:FnOnce(&SkCanvas)
  {
    if let Some(canvas) = self.current.recording_canvas() {
      f(canvas);
      self.changed = true;
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
    if let Some(canvas) = self.current.recording_canvas() {
      canvas.set_matrix(&matrix.into());
    }
  }

  pub fn set_clip(&mut self, clip:&Option<Path>){
    self.clip = clip.clone();
    self.restore();
  }

  pub fn restore(&mut self){
    if let Some(canvas) = self.current.recording_canvas() {
      canvas.restore_to_count(1);
      canvas.save();
      if let Some(clip) = &self.clip{
        canvas.clip_path(clip, ClipOp::Intersect, true /* antialias */);
      }
      canvas.set_matrix(&self.matrix.into());
    }
  }

  pub fn get_pixels(&mut self, crop:IRect, opts:ExportOptions, engine:RenderingEngine) -> Result<Vec<u8>, String>{
    let dst_info = ImageInfo::new((crop.width(), crop.height()), opts.color_type.clone(), AlphaType::Unpremul, opts.color_space.clone());
    let src_size = Size::new(self.bounds.width() * opts.density, self.bounds.height() * opts.density);
    let src_info = ImageInfo::new_n32_premul(src_size.to_floor(), dst_info.color_space());
    let page = self.get_page();
    let depth = page.layers.len();

    engine.with_surface(&src_info, &opts.clone(), |surface| {
      let (cache_image, cache_depth) = PageCache::read(self.id, &opts, depth);

      let canvas = surface.canvas();
      if let Some(image) = cache_image{
        // use the cached bitmap as the background (if present)
        canvas.draw_image(image, (0,0), None);
      }else if let Some(color) = opts.matte{
        // otherwise, fill the canvas if requested
        canvas.clear(color);
      }

      // draw newly added layers and cache the full-canvas bitmap
      canvas.scale((opts.density, opts.density));
      for pict in page.layers.iter().skip(cache_depth){
        pict.playback(canvas);
      }
      PageCache::write(self.id, &surface.image_snapshot(), &opts, depth);

      // use cached image (reading just the pixels in the requested rect)
      let mut dst_buffer: Vec<u8> = vec![0; dst_info.compute_min_byte_size()];
      match PageCache::copy_pixels(self.id, surface, &dst_info, crop, &mut dst_buffer) {
        true => Ok(dst_buffer),
        false => Err(format!("Could not get image data (format: {:?})", dst_info.color_type()))
      }
    })
  }

  pub fn get_page(&mut self) -> Page{
    if self.changed {
      // stop and restart the recorder while adding its content as a new layer
      if let Some(palimpsest) = self.current.finish_recording_as_picture(Some(&self.bounds)) {
        self.layers.push(palimpsest);
      }
      self.current.begin_recording(self.bounds, None);
      self.changed = false;
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
        images::deferred_from_picture(
          pict, size, None, None, BitDepth::U8, Some(ColorSpace::new_srgb()), None
        )
      })
  }
}

impl Drop for PageRecorder{
  fn drop(&mut self) {
    PageCache::drop(self.id);
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
    self.layers.len() == other.layers.len()
  }
}

impl Default for Page {
  fn default() -> Self {
    Self{ id:0, bounds: skia_safe::Rect::new_empty(), layers:vec![] }
  }
}

impl Page{

  pub fn get_picture(&self, matte:Option<Color>) -> Option<Picture> {
    let mut compositor = PictureRecorder::new();
    compositor.begin_recording(self.bounds, None);
    if let Some(output) = compositor.recording_canvas() {
      matte.map(|c| output.clear(c));
      for pict in self.layers.iter(){
        pict.playback(output);
      }
    }
    compositor.finish_recording_as_picture(Some(&self.bounds))
  }

  pub fn encoded_as(&self, options:ExportOptions, engine:RenderingEngine) -> Result<Vec<u8>, String> {
    if self.bounds.is_empty(){
      return Err("Width and height must be non-zero to generate an image".to_string())
    }

    let ExportOptions{ ref format, quality, density, matte, color_type, .. } = options;
    let size = self.bounds.size();
    let img_dims = Size::new(size.width * density, size.height * density).to_floor();
    let img_info = ImageInfo::new_n32_premul(img_dims, Some(ColorSpace::new_srgb()));
    let img_quality = ((quality*100.0) as u32).clamp(0, 100);
    let img_scale = Matrix::scale((density, density)).into();

    match format.as_str(){
      "pdf" => {
        let mut pdf_bytes = Vec::new();
        let mut document = pdf_document(&mut pdf_bytes, quality, density).begin_page(size, None);
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
      _ => engine.with_surface(&img_info, &options, |surface|{
        let canvas = surface.canvas();
        let depth = self.layers.len();
        let (cache_image, cache_depth) = PageCache::read(self.id, &options, depth);

        if let Some(image) = cache_image{
          // use the cached bitmap as the background
          canvas.draw_image(image, (0,0), None);
        }else if let Some(color) = options.matte{
          // otherwise, fill the canvas if requested
          canvas.clear(color);
        }

        // draw newly added layers and cache the full-canvas bitmap
        canvas.set_matrix(&img_scale);
        for pict in self.layers.iter().skip(cache_depth){
          pict.playback(canvas);
        }

        // extract the results
        let mut image = surface.image_snapshot();
        let context = &mut surface.direct_context();

        // update cache
        if rayon::current_thread_index().is_some(){
          // only move off GPU if we're in a background thread and need to share
          let context = &mut surface.direct_context();
          if let Some(raster) = image.make_raster_image(context, CachingHint::Allow){
            image = raster;
          }
        }
        PageCache::write(self.id, &image, &options, depth);

        match format.as_str(){
          "raw" => {
            let dst_info = ImageInfo::new(img_dims, color_type, AlphaType::Unpremul, Some(ColorSpace::new_srgb()));
            let mut buffer: Vec<u8> = vec![0; dst_info.compute_min_byte_size()];
            match surface.read_pixels(&dst_info, &mut buffer, dst_info.min_row_bytes(), (0,0)){
              true => Some(buffer),
              false => return Err(format!("Could not encode as {} ({:?})", format, color_type))
            }
          }

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
            let png_opts = png_encoder::Options::default();

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
      })
    }
  }

  pub fn write(&self, filename: &str, options:ExportOptions, engine:RenderingEngine) -> Result<(), String> {
    let path = FilePath::new(&filename);
    let data = self.encoded_as(options, engine)?;
    fs::write(path, data).map_err(|why|
      format!("{}: \"{}\"", why, path.display())
    )
  }

  fn append_to<'a>(&self, doc:Document<'a>, matte:Option<Color>) -> Result<Document<'a>, String>{
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

  pub fn materialize(&mut self, engine:&RenderingEngine, options:&ExportOptions){
    if !options.is_raster(){ return }
    for page in self.pages.iter_mut(){
      PageCache::materialize(page.id, &engine, &options);
    }
  }

  pub fn as_pdf(&self, options:ExportOptions) -> Result<Vec<u8>, String>{
    let ExportOptions{ quality, density, matte, .. } = options;
    let mut pdf_bytes = Vec::new();
    self.pages
      .iter()
      .try_fold(pdf_document(&mut pdf_bytes, quality, density), |doc, page| page.append_to(doc, matte))
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
// Cache for the last bitmap generated by a given Page
//

#[derive(Debug, Clone)]
struct PageCache{
  image: Option<SkImage>,
  density: f32,
  matte: Option<Color>,
  msaa: Option<usize>,
  depth: usize,
}

impl Default for PageCache{
  fn default() -> Self {
    Self{image:None, density:1.0, matte:None, msaa:None, depth:0}
  }
}

impl PageCache{
  pub fn shared<'a>() -> &'a Arc<DashMap<usize, PageCache>>{
    CACHE.get_or_init(|| Arc::new(DashMap::new()))
  }

  pub fn add(id:usize){
    Self::shared().insert(id, PageCache::default());
  }

  pub fn drop(id:usize){
    Self::shared().remove(&id).unwrap();
  }

  pub fn read(id:usize, opts:&ExportOptions, depth:usize) -> (Option<SkImage>, usize){
    Self::shared().get(&id).map(|cache|{
      let is_valid = opts.is_raster() &&
        opts.density == cache.density &&
        opts.matte == cache.matte &&
        opts.msaa == cache.msaa &&
        depth >= cache.depth &&
        cache.image.is_some();

      match is_valid{
        true => (cache.image.clone(), cache.depth),
        false => (None, 0)
      }
    })
    .unwrap_or((None, 0))
  }

  pub fn write(id:usize, image:&SkImage, opts:&ExportOptions, depth:usize){
    Self::shared().get_mut(&id).map(|mut cache|{
      let out_of_order = cache.image.is_some() &&
          opts.density == cache.density &&
          opts.matte == cache.matte &&
          opts.msaa == cache.msaa &&
          depth <= cache.depth;

      if !out_of_order{
        *cache = Self{ image:Some(image.clone()), density:opts.density, matte:opts.matte, msaa:opts.msaa, depth}
      }
    });
  }

  pub fn materialize(id:usize, engine:&RenderingEngine, options:&ExportOptions){
    Self::shared().get_mut(&id).map(|mut cache|{
      if let Some(ref img) = cache.image{
        // nothing to be done if the image isn't currently in GPU memory
        if !img.is_texture_backed(){ return }

        // otherwise move the image to main memory
        cache.image = cache.image.as_ref().and_then(|image|
          engine.with_surface(image.image_info(), options, |surface|{
            surface.direct_context().as_mut()
              .and_then(|context|
                image.make_raster_image(context, CachingHint::Allow)
              ).ok_or(String::from("Failed"))
          }).ok()
        );
      }
    });
  }

  pub fn copy_pixels(id:usize, surface: &mut Surface, dst_info: &ImageInfo, src: IRect, pixels: &mut [u8]) -> bool {
    Self::shared().get(&id).and_then(|cache|{
      cache._blit(surface, dst_info, src, pixels)
    }).unwrap_or(false)
  }

  #[cfg(not(any(feature="metal", feature="vulkan")))]
  fn _blit<'a>( &self, _surface: &mut Surface, dst_info: &ImageInfo, src: IRect, pixels: &mut [u8], ) -> Option<bool>{
    self.image.as_ref().map(|image| image.read_pixels(
      &dst_info, pixels, dst_info.min_row_bytes(), (src.x(), src.y()), CachingHint::Allow
    ))
  }

  #[cfg(any(feature="metal", feature="vulkan"))]
  fn _blit<'a>( &self, surface: &mut Surface, dst_info: &ImageInfo, src: IRect, pixels: &mut [u8], ) -> Option<bool>{
    let context = &mut surface.direct_context();
    self.image.as_ref().map(|image| image.read_pixels_with_context(
      context, &dst_info, pixels, dst_info.min_row_bytes(), (src.x(), src.y()), CachingHint::Allow
    ))
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

fn pdf_document(buffer:&mut impl std::io::Write, quality:f32, density:f32) -> Document{
  pdf::new_document(buffer, Some(&pdf::Metadata {
    producer: "Skia Canvas <https://skia-canvas.org>".to_string(),
    encoding_quality: Some((quality*100.0) as i32),
    raster_dpi: Some(density * 72.0),
    ..Default::default()
  }))
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExportOptions{
  pub format: String,
  pub quality: f32,
  pub density: f32,
  pub outline: bool,
  pub matte: Option<Color>,
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

  pub fn msaa_from(&self, valid_msaa:&Vec<usize>) -> Result<usize, String>{
    let samples = self.msaa.unwrap_or_else(||
      if valid_msaa.contains(&4){ 4 } // 4x is a good default if available
      else{ *valid_msaa.last().unwrap() }
    );
    match valid_msaa.contains(&samples){
      true => Ok(samples),
      false => Err(format!("{}x MSAA not supported by GPU (options: {:?})", samples, valid_msaa))
    }
  }

  pub fn is_raster(&self) -> bool{
    self.format!="pdf" && self.format!="svg"
  }
}