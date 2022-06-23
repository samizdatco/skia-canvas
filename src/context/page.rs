use std::fs;
use std::path::Path as FilePath;
use rayon::prelude::*;
use neon::prelude::*;
use neon::result::Throw;
use skia_safe::{Canvas as SkCanvas, Path, Matrix, Rect, ClipOp, Size, Data, Color, ColorSpace,
                PictureRecorder, Picture, Surface, EncodedImageFormat, Image as SkImage, ColorType,
                svg::{self, canvas::Flags}, pdf, Document, ImageInfo, Budgeted,
                gpu::{SurfaceOrigin, DirectContext},
                image::BitDepth};
use surfman::{Device, Context, Connection, ContextAttributeFlags, ContextAttributes, GLApi, GLVersion};

use crc::{Crc, CRC_32_ISO_HDLC};
const CRC32: Crc<u32> = Crc::<u32>::new(&CRC_32_ISO_HDLC);

use crate::context::BoxedContext2D;

//
// Deferred canvas (records drawing commands for later replay on an output surface)
//

pub struct PageRecorder{
  current: PictureRecorder,
  layers: Vec<Picture>,
  cache: Option<SkImage>,
  bounds: Rect,
  matrix: Matrix,
  clip: Option<Path>,
  changed: bool,
}

impl PageRecorder{
  pub fn new(bounds:Rect) -> Self {
    let mut rec = PictureRecorder::new();
    rec.begin_recording(bounds, None);
    rec.recording_canvas().unwrap().save(); // start at depth 2
    PageRecorder{ current:rec, changed:false, layers:vec![], cache:None, matrix:Matrix::default(), clip:None, bounds }
  }

  pub fn append<F>(&mut self, f:F)
    where F:FnOnce(&mut SkCanvas)
  {
    if let Some(canvas) = self.current.recording_canvas() {
      f(canvas);
      self.changed = true;
    }
  }

  pub fn set_bounds(&mut self, bounds:Rect){
    *self = PageRecorder::new(bounds);
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
        canvas.clip_path(&clip, ClipOp::Intersect, true /* antialias */);
      }
      canvas.set_matrix(&self.matrix.into());
    }
  }

  pub fn get_page(&mut self) -> Page{
    if self.changed {
      // stop and restart the recorder while adding its content as a new layer
      if let Some(palimpsest) = self.current.finish_recording_as_picture(Some(&self.bounds)) {
        self.layers.push(palimpsest);
      }
      self.current.begin_recording(self.bounds, None);
      self.changed = false;
      self.cache = None;
      self.restore();
    }

    Page{
      layers: self.layers.clone(),
      bounds: self.bounds,
    }
  }

  pub fn get_image(&mut self) -> Option<SkImage>{
    let page = self.get_page();
    if self.cache.is_none(){
      if let Some(pict) = page.get_picture(None){
        let size = page.bounds.size().to_floor();
        self.cache = SkImage::from_picture(pict, size, None, None, BitDepth::U8, Some(ColorSpace::new_srgb()));
      }
    }
    self.cache.clone()
  }
}

//
// Image generator for a single drawing context
//

use std::cell::RefCell;
use crate::init_gpu::get_vulkan_context;
thread_local!(static GL_CONTEXT: RefCell<Option<GLContext>> = RefCell::new(None));

fn gl_init(){
  GL_CONTEXT.with(|cell| {
    let mut local_ctx = cell.borrow_mut();
    if local_ctx.is_none(){
      local_ctx.replace(GLContext::new());
    }
  });
}

struct GLContext {
  device:Device,
  context:Context
}

impl GLContext {
  pub fn new() -> Self{
    let connection = Connection::new().unwrap();
    let adapter = connection.create_hardware_adapter().unwrap();
    let mut device = connection.create_device(&adapter).unwrap();
    let context_attributes = ContextAttributes {
      version: GLVersion::new(3, 3),
      flags: ContextAttributeFlags::empty(),
    };
    let context_descriptor = device
        .create_context_descriptor(&context_attributes)
        .unwrap();
    let mut context = device.create_context(&context_descriptor, None).unwrap();
    device.make_context_current(&context).unwrap();
    gl::load_with(|symbol_name| device.get_proc_address(&context, symbol_name));

    GLContext{device, context}
  }
}

impl Drop for GLContext {
  fn drop(&mut self) {
    self.device.destroy_context(&mut self.context).unwrap();
  }
}
pub struct Page{
  pub layers: Vec<Picture>,
  pub bounds: Rect,
}

impl Page{

  pub fn get_picture(&self, matte:Option<Color>) -> Option<Picture> {
    let mut compositor = PictureRecorder::new();
    compositor.begin_recording(self.bounds, None);
    if let Some(output) = compositor.recording_canvas() {
      output.clear(matte.unwrap_or(Color::TRANSPARENT));
      for pict in self.layers.iter(){
        pict.playback(output);
      }
    }
    compositor.finish_recording_as_picture(Some(&self.bounds))
  }

  pub fn encoded_as(&self, format:&str, quality:f32, density:f32, outline:bool, matte:Option<Color>) -> Result<Data, String> {
    self.gl_encoded_as(&format, quality, density, outline, matte)
    // self.cpu_encoded_as(&format, quality, density, outline, matte)
  }

  pub fn gl_encoded_as(&self, format:&str, quality:f32, density:f32, outline:bool, matte:Option<Color>) -> Result<Data, String> {
    let picture = self.get_picture(matte).ok_or("Could not generate an image")?;

    if self.bounds.is_empty(){
      Err("Width and height must be non-zero to generate an image".to_string())
    }else{
      let img_dims = self.bounds.size();
      let img_format = match format {
        "jpg" | "jpeg" => Some(EncodedImageFormat::JPEG),
        "png" => Some(EncodedImageFormat::PNG),
        _ => None
      };

      if let Some(img_format) = img_format{
        gl_init();
        let mut gl_context = DirectContext::new_gl(None, None).unwrap();
        let img_scale = Matrix::scale((density, density));
        let img_dims = Size::new(img_dims.width * density, img_dims.height * density).to_floor();
        let img_info = ImageInfo::new_n32_premul(img_dims, None);

        let mut surface = Surface::new_render_target(
          &mut get_vulkan_context(),
          Budgeted::Yes,
          &img_info,
          Some(4),
          SurfaceOrigin::BottomLeft,
          None,
          true,
        );

        if let Some(mut surface) = surface{
          surface
            .canvas()
            .clear(matte.unwrap_or(Color::TRANSPARENT))
            .set_matrix(&img_scale.into())
            .draw_picture(&picture, None, None);
          surface
            .image_snapshot()
            .encode_to_data_with_quality(img_format, (quality*100.0) as i32)
            .map(|data| with_dpi(data, img_format, density))
            .ok_or(format!("Could not encode as {}", format))
        }else{
          Err("Could not allocate new bitmap".to_string())
        }
      }else if format == "pdf"{
        let mut document = pdf_document(quality, density).begin_page(img_dims, None);
        let canvas = document.canvas();
        canvas.draw_picture(&picture, None, None);
        Ok(document.end_page().close())
      }else if format == "svg"{
        let flags = outline.then(|| Flags::CONVERT_TEXT_TO_PATHS);
        let mut canvas = svg::Canvas::new(Rect::from_size(img_dims), flags);
        canvas.draw_picture(&picture, None, None);
        Ok(canvas.end())
      }else{
        Err(format!("Unsupported file format {}", format))
      }

    }

  }

  pub fn cpu_encoded_as(&self, format:&str, quality:f32, density:f32, outline:bool, matte:Option<Color>) -> Result<Data, String> {
    let picture = self.get_picture(matte).ok_or("Could not generate an image")?;

    if self.bounds.is_empty(){
      Err("Width and height must be non-zero to generate an image".to_string())
    }else{
      let img_dims = self.bounds.size();
      let img_format = match format {
        "jpg" | "jpeg" => Some(EncodedImageFormat::JPEG),
        "png" => Some(EncodedImageFormat::PNG),
        _ => None
      };

      if let Some(img_format) = img_format{
        let img_scale = Matrix::scale((density, density));
        let img_dims = Size::new(img_dims.width * density, img_dims.height * density).to_floor();
        if let Some(img) = SkImage::from_picture(picture, img_dims, Some(&img_scale), None, BitDepth::U8, Some(ColorSpace::new_srgb())){
          img
            .encode_to_data_with_quality(img_format, (quality*100.0) as i32)
            .map(|data| with_dpi(data, img_format, density))
            .ok_or(format!("Could not encode as {}", format))
        }else{
          Err("Could not allocate new bitmap".to_string())
        }
      }else if format == "pdf"{
        let mut document = pdf_document(quality, density).begin_page(img_dims, None);
        let canvas = document.canvas();
        canvas.draw_picture(&picture, None, None);
        Ok(document.end_page().close())
      }else if format == "svg"{
        let flags = outline.then(|| Flags::CONVERT_TEXT_TO_PATHS);
        let mut canvas = svg::Canvas::new(Rect::from_size(img_dims), flags);
        canvas.draw_picture(&picture, None, None);
        Ok(canvas.end())
      }else{
        Err(format!("Unsupported file format {}", format))
      }

    }

  }

  pub fn write(&self, filename: &str, file_format:&str, quality:f32, density:f32, outline:bool, matte:Option<Color>) -> Result<(), String> {
    let path = FilePath::new(&filename);
    let data = self.encoded_as(file_format, quality, density, outline, matte)?;
    fs::write(path, data.as_bytes()).map_err(|why|
      format!("{}: \"{}\"", why, path.display())
    )
  }

  fn append_to(&self, doc:Document, matte:Option<Color>) -> Result<Document, String>{
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
  pages: Vec<Page>
}

impl PageSequence{
  pub fn from(pages:Vec<Page>) -> Self{
    PageSequence { pages }
  }

  pub fn first(&self) -> &Page {
    &self.pages[0]
  }

  pub fn len(&self) -> usize{
    self.pages.len()
  }

  pub fn as_pdf(&self, quality:f32, density:f32, matte:Option<Color>) -> Result<Data, String>{
    self.pages
      .iter()
      .try_fold(pdf_document(quality, density), |doc, page| page.append_to(doc, matte))
      .map(|doc| doc.close())
  }

  #[allow(clippy::too_many_arguments)]
  pub fn write_sequence(&self, pattern:&str, format:&str, padding:f32, quality:f32, density:f32, outline:bool, matte:Option<Color>) -> Result<(), String>{
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
        page.write(&filename, format, quality, density, outline, matte)
      })
  }

  pub fn write_pdf(&self, path:&str, quality:f32, density:f32, matte:Option<Color>) -> Result<(), String>{
    let path = FilePath::new(&path);
    match self.as_pdf(quality, density, matte){
      Ok(document) => fs::write(path, document.as_bytes()).map_err(|why|
        format!("{}: \"{}\"", why, path.display())
      ),
      Err(msg) => Err(msg)
    }
  }

}

//
// Helpers
//

pub fn pages_arg(cx: &mut FunctionContext, idx: i32) -> Result<PageSequence, Throw> {
  let pages = cx.argument::<JsArray>(idx)?
      .to_vec(cx)?
      .iter()
      .map(|obj| obj.downcast::<BoxedContext2D, _>(cx))
      .filter( |ctx| ctx.is_ok() )
      .map(|obj| obj.unwrap().borrow().get_page())
      .collect();
  Ok(PageSequence::from(pages))
}

fn pdf_document(quality:f32, density:f32) -> Document{
  let mut meta = pdf::Metadata::default();
  meta.producer = "Skia Canvas <https://github.com/samizdatco/skia-canvas>".to_string();
  meta.encoding_quality = Some((quality*100.0) as i32);
  meta.raster_dpi = Some(density * 72.0);
  pdf::new_document(Some(&meta))
}

fn with_dpi(data:Data, format:EncodedImageFormat, density:f32) -> Data{
  if density as u32 == 1 { return data }

  let mut bytes = data.as_bytes().to_vec();
  match format{
    EncodedImageFormat::JPEG => {
      let [l, r] = (72 * density as u16).to_be_bytes();
      bytes.splice(13..18, [1, l, r, l, r].iter().cloned());
      Data::new_copy(&bytes)
    }
    EncodedImageFormat::PNG => {
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
      Data::new_copy(&bytes)
    }
    _ => data
  }
}
