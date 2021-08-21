#![allow(non_snake_case)]
#![allow(clippy::too_many_arguments)]
use std::fs;
use std::path::Path;
use std::cell::RefCell;
use std::sync::{Arc, Mutex};
use neon::prelude::*;
use rayon::prelude::*;
use crc::{crc32, Hasher32};
use skia_safe::{Rect, Matrix, Path as SkPath, Picture, PictureRecorder,
                Size, ClipOp, Surface, EncodedImageFormat, Data, Color,
                svg::{self, canvas::Flags}, Document};

use crate::utils::*;
use crate::context::BoxedContext2D;

pub type BoxedCanvas = JsBox<RefCell<Canvas>>;
impl Finalize for Canvas {}

pub struct Canvas{
  pub width: f32,
  pub height: f32,
  async_io: bool,
}

impl Canvas{
  pub fn new() -> Self{
    Canvas{width:300.0, height:150.0, async_io:true}
  }
}

//
// -- File I/O ------------------------------------------------------------------------------------
//

pub struct Page{
  pub recorder: Arc<Mutex<PictureRecorder>>,
  pub bounds: Rect,
  pub clip: SkPath,
  pub matrix: Matrix,
}

unsafe impl Send for Page{}
unsafe impl Sync for Page{}

impl Page{

  fn get_picture(&self) -> Option<Picture> {
    // stop the recorder to take a snapshot then restart it again
    let recorder = Arc::clone(&self.recorder);
    let mut recorder = recorder.lock().unwrap();
    let snapshot = recorder.finish_recording_as_picture(Some(&self.bounds));
    recorder.begin_recording(self.bounds, None);

    if let Some(canvas) = recorder.recording_canvas() {
      // fill the newly restarted recorder with the snapshot content...
      if let Some(palimpsest) = &snapshot {
        canvas.draw_picture(&palimpsest, None, None);
      }

      // ...and the current ctm/clip state
      canvas.save();
      canvas.set_matrix(&self.matrix.into());
      if !self.clip.is_empty(){
        canvas.clip_path(&self.clip, ClipOp::Intersect, true /* antialias */);
      }
    }
    snapshot
  }

  fn encoded_as(&self, format:&str, quality:f32, density:f32, outline:bool, matte:Option<Color>) -> Result<Data, String> {
    let picture = self.get_picture().ok_or("Could not generate an image")?;

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
        if let Some(mut surface) = Surface::new_raster_n32_premul(img_dims){
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

  fn write(&self, filename: &str, file_format:&str, quality:f32, density:f32, outline:bool, matte:Option<Color>) -> Result<(), String> {
    let path = Path::new(&filename);
    let data = self.encoded_as(&file_format, quality, density, outline, matte)?;
    fs::write(path, data.as_bytes()).map_err(|why|
      format!("{}: \"{}\"", why, path.display())
    )
  }

  fn append_to(&self, doc:Document) -> Result<Document, String>{
    if !self.bounds.is_empty(){
      let mut doc = doc.begin_page(self.bounds.size(), None);
      let canvas = doc.canvas();
      if let Some(picture) = self.get_picture(){
        canvas.draw_picture(&picture, None, None);
      }
      Ok(doc.end_page())
    }else{
      Err("Width and height must be non-zero to generate a PDF page".to_string())
    }
  }
}

fn to_pdf(pages:&[Page], quality:f32, density:f32) -> Result<Data, String>{
  pages
    .iter()
    .try_fold(pdf_document(quality, density), |doc, page| page.append_to(doc))
    .map(|doc| doc.close())
}

fn write_pdf(path:&str, pages:&[Page], quality:f32, density:f32) -> Result<(), String>{
  let path = Path::new(&path);
  match to_pdf(&pages, quality, density){
    Ok(document) => fs::write(path, document.as_bytes()).map_err(|why|
      format!("{}: \"{}\"", why, path.display())
    ),
    Err(msg) => Err(msg)
  }
}

fn write_sequence(pages:&[Page], pattern:&str, format:&str, padding:f32, quality:f32, density:f32, outline:bool, matte:Option<Color>) -> Result<(), String>{
  let padding = match padding as i32{
    -1 => (1.0 + (pages.len() as f32).log10().floor()) as usize,
    pad => pad as usize
  };

  pages
    .par_iter()
    .enumerate()
    .try_for_each(|(pp, page)|{
      let folio = format!("{:0width$}", pp+1, width=padding);
      let filename = pattern.replace("{}", folio.as_str());
      page.write(&filename, &format, quality, density, outline, matte)
    })
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
      let mut digest = crc32::Digest::new(crc32::IEEE);
      let [a, b, c, d] = ((72.0 * density * 39.3701) as u32).to_be_bytes();
      let phys = vec![
        b'p', b'H', b'Y', b's',
        a, b, c, d, // x-dpi
        a, b, c, d, // y-dpi
        1, // dots per meter
      ];
      digest.write(&phys);

      let length = 9u32.to_be_bytes().to_vec();
      let checksum = digest.sum32().to_be_bytes().to_vec();
      bytes.splice(33..33, [length, phys, checksum].concat());
      Data::new_copy(&bytes)
    }
    _ => data
  }
}

use neon::result::Throw;
fn pages_arg(cx: &mut FunctionContext, idx: i32) -> Result<Vec<Page>, Throw> {
  let pages = cx.argument::<JsArray>(idx)?
      .to_vec(cx)?
      .iter()
      .map(|obj| obj.downcast::<BoxedContext2D, _>(cx))
      .filter( |ctx| ctx.is_ok() )
      .map(|obj| obj.unwrap().borrow().get_page())
      .collect::<Vec<Page>>();
  Ok(pages)
}


//
// -- Javascript Methods --------------------------------------------------------------------------
//

pub fn new(mut cx: FunctionContext) -> JsResult<BoxedCanvas> {
  let this = RefCell::new(Canvas::new());
  Ok(cx.boxed(this))
}

pub fn get_width(mut cx: FunctionContext) -> JsResult<JsNumber> {
  let this = cx.argument::<BoxedCanvas>(0)?;
  let width = this.borrow().width;
  Ok(cx.number(width as f64))
}

pub fn get_height(mut cx: FunctionContext) -> JsResult<JsNumber> {
  let this = cx.argument::<BoxedCanvas>(0)?;
  let height = this.borrow().height;
  Ok(cx.number(height as f64))
}

pub fn set_width(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedCanvas>(0)?;
  let width = float_arg(&mut cx, 1, "size")?;
  this.borrow_mut().width = width;
  Ok(cx.undefined())
}

pub fn set_height(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedCanvas>(0)?;
  let height = float_arg(&mut cx, 1, "size")?;
  this.borrow_mut().height = height;
  Ok(cx.undefined())
}

pub fn get_async(mut cx: FunctionContext) -> JsResult<JsBoolean> {
  let this = cx.argument::<BoxedCanvas>(0)?;
  let this = this.borrow();
  Ok(cx.boolean(this.async_io))
}

pub fn set_async(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedCanvas>(0)?;
  let go_async = cx.argument::<JsBoolean>(1)?;
  this.borrow_mut().async_io = go_async.value(&mut cx);
  Ok(cx.undefined())
}

pub fn toBuffer(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  // let this = cx.argument::<BoxedCanvas>(0)?;
  let callback = cx.argument::<JsFunction>(1)?.root(&mut cx);
  let pages = pages_arg(&mut cx, 2)?;
  let file_format = string_arg(&mut cx, 3, "format")?;
  let quality = float_arg(&mut cx, 4, "quality")?;
  let density = float_arg(&mut cx, 5, "density")?;
  let outline = bool_arg(&mut cx, 6, "outline")?;
  let matte = color_arg(&mut cx, 7);
  let channel = cx.channel();

  rayon::spawn(move || {
    let encoded = {
      if file_format=="pdf" && pages.len() > 1 {
        to_pdf(&pages, quality, density)
      }else{
        pages[0].encoded_as(&file_format, quality, density, outline, matte)
      }
    };

    channel.send(move |mut cx| {
      let callback = callback.into_inner(&mut cx);
      let this = cx.undefined();

      let args = match encoded{
        Ok(data) => {
          let mut buffer = JsBuffer::new(&mut cx, data.len() as u32).unwrap();
          cx.borrow_mut(&mut buffer, |buf_data| {
            buf_data.as_mut_slice().copy_from_slice(&data);
          });
          vec![
            cx.string("ok").upcast::<JsValue>(),
            buffer.upcast::<JsValue>(),
          ]
        },
        Err(msg) => vec![
          cx.string("err").upcast::<JsValue>(),
          cx.string(msg).upcast::<JsValue>(),
        ]
      };

      callback.call(&mut cx, this, args)?;
      Ok(())
    });
  });

  Ok(cx.undefined())
}

pub fn toBufferSync(mut cx: FunctionContext) -> JsResult<JsValue> {
  // let this = cx.argument::<BoxedCanvas>(0)?;
  let pages = pages_arg(&mut cx, 1)?;
  let file_format = string_arg(&mut cx, 2, "format")?;
  let quality = float_arg(&mut cx, 3, "quality")?;
  let density = float_arg(&mut cx, 4, "density")?;
  let outline = bool_arg(&mut cx, 5, "outline")?;
  let matte = color_arg(&mut cx, 6);

    let encoded = {
      if file_format=="pdf" && pages.len() > 1 {
        to_pdf(&pages, quality, density)
      }else{
        pages[0].encoded_as(&file_format, quality, density, outline, matte)
      }
    };

    match encoded{
      Ok(data) => {
        let mut buffer = JsBuffer::new(&mut cx, data.len() as u32).unwrap();
        cx.borrow_mut(&mut buffer, |buf_data| {
          buf_data.as_mut_slice().copy_from_slice(&data);
        });
        Ok(buffer.upcast::<JsValue>())
      },
      Err(msg) => cx.throw_error(msg)
    }
}


pub fn save(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  // let this = cx.argument::<BoxedCanvas>(0)?;
  let callback = cx.argument::<JsFunction>(1)?.root(&mut cx);
  let pages = pages_arg(&mut cx, 2)?;
  let name_pattern = string_arg(&mut cx, 3, "filePath")?;
  let sequence = !cx.argument::<JsValue>(4)?.is_a::<JsUndefined, _>(&mut cx);
  let padding = opt_float_arg(&mut cx, 4).unwrap_or(-1.0);
  let file_format = string_arg(&mut cx, 5, "format")?;
  let quality = float_arg(&mut cx, 6, "quality")?;
  let density = float_arg(&mut cx, 7, "density")?;
  let outline = bool_arg(&mut cx, 8, "outline")?;
  let matte = color_arg(&mut cx, 9);
  let channel = cx.channel();

  rayon::spawn(move || {
    let result = {
      if sequence {
        write_sequence(&pages, &name_pattern, &file_format, padding, quality, density, outline, matte)
      } else if file_format == "pdf" {
        write_pdf(&name_pattern, &pages, quality, density)
      } else {
        pages[0].write(&name_pattern, &file_format, quality, density, outline, matte)
      }
    };

    channel.send(move |mut cx| {
      let callback = callback.into_inner(&mut cx);
      let this = cx.undefined();
      let args = match result {
        Ok(_) => vec![
          cx.string("ok").upcast::<JsValue>(),
          cx.undefined().upcast::<JsValue>(),
        ],
        Err(msg) => vec![
          cx.string("err").upcast::<JsValue>(),
          cx.string(msg).upcast::<JsValue>(),
        ]
      };

      callback.call(&mut cx, this, args)?;
      Ok(())
    });
  });

  Ok(cx.undefined())
}

pub fn saveSync(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  // let this = cx.argument::<BoxedCanvas>(0)?;
  let pages = pages_arg(&mut cx, 1)?;
  let name_pattern = string_arg(&mut cx, 2, "filePath")?;
  let sequence = !cx.argument::<JsValue>(3)?.is_a::<JsUndefined, _>(&mut cx);
  let padding = opt_float_arg(&mut cx, 3).unwrap_or(-1.0);
  let file_format = string_arg(&mut cx, 4, "format")?;
  let quality = float_arg(&mut cx, 5, "quality")?;
  let density = float_arg(&mut cx, 6, "density")?;
  let outline = bool_arg(&mut cx, 7, "outline")?;
  let matte = color_arg(&mut cx, 8);

  let result = {
    if sequence {
      write_sequence(&pages, &name_pattern, &file_format, padding, quality, density, outline, matte)
    } else if file_format == "pdf" {
      write_pdf(&name_pattern, &pages, quality, density)
    } else {
      pages[0].write(&name_pattern, &file_format, quality, density, outline, matte)
    }
  };

  match result{
    Ok(_) => Ok(cx.undefined()),
    Err(msg) => cx.throw_error(msg)
  }
}
