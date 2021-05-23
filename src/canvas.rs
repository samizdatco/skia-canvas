#![allow(non_snake_case)]
use std::fs;
use std::path::Path;
use std::cell::RefCell;
use std::sync::{Arc, Mutex};
use neon::prelude::*;
use skia_safe::{Rect, Matrix, Path as SkPath, Picture, PictureRecorder,
                ISize, ClipOp, Surface, EncodedImageFormat, Data,
                pdf, svg, Document};

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
  this.borrow_mut().width = width.floor();
  Ok(cx.undefined())
}

pub fn set_height(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedCanvas>(0)?;
  let height = float_arg(&mut cx, 1, "size")?;
  this.borrow_mut().height = height.floor();
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

  fn encoded_as(&self, format:&str, quality: f32) -> Result<Data, String> {
    let picture = self.get_picture().ok_or("Could not generate an image")?;

    if self.bounds.is_empty(){
      Err("Width and height must be non-zero to generate an image".to_string())
    }else{
      let img_dims:ISize = self.bounds.size().to_floor();
      let img_format = match format {
        "jpg" | "jpeg" => Some(EncodedImageFormat::JPEG),
        "png" => Some(EncodedImageFormat::PNG),
        _ => None
      };

      if let Some(img_format) = img_format{
        if let Some(mut surface) = Surface::new_raster_n32_premul(img_dims){
          surface.canvas().draw_picture(&picture, None, None);
          let img = surface.image_snapshot();
          img.encode_to_data_with_quality(img_format, quality as i32)
             .ok_or(format!("Could not encode as {}", format))
        }else{
          Err("Could not allocate new bitmap".to_string())
        }
      }else if format == "pdf"{
        let mut document = pdf::new_document(None).begin_page(img_dims, None);
        let canvas = document.canvas();
        canvas.draw_picture(&picture, None, None);
        Ok(document.end_page().close())
      }else if format == "svg"{
        let mut canvas = svg::Canvas::new(Rect::from_size(img_dims), None);
        canvas.draw_picture(&picture, None, None);
        Ok(canvas.end())
      }else{
        Err(format!("Unsupported file format {}", format))
      }

    }

  }

  fn write(&self, filename: &str, file_format:&str, quality: f32) -> Result<(), String> {
    let path = Path::new(&filename);
    let data = self.encoded_as(&file_format, quality)?;
    fs::write(path, data.as_bytes()).map_err(|why|
      format!("{}: \"{}\"", why, path.display())
    )
  }

  fn append_to(&self, doc:Document) -> Result<Document, String>{
    if !self.bounds.is_empty(){
      let dims = (self.bounds.width() as i32, self.bounds.height() as i32);
      let mut doc = doc.begin_page(dims, None);
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


fn to_pdf(pages:&[Page]) -> Result<Data, String>{
  pages
    .iter()
    .rev()
    .try_fold(pdf::new_document(None), |doc, page| page.append_to(doc))
    .map(|doc| doc.close())
}

fn write_pdf(path:&str, pages:&[Page]) -> Result<(), String>{
  let path = Path::new(&path);
  match to_pdf(&pages){
    Ok(document) => fs::write(path, document.as_bytes()).map_err(|why|
      format!("{}: \"{}\"", why, path.display())
    ),
    Err(msg) => Err(msg)
  }
}

fn write_sequence(pages:&[Page], pattern:&str, format:&str, padding:f32, quality:f32) -> Result<(), String>{
  let padding = match padding as i32{
    -1 => (1.0 + (pages.len() as f32).log10().floor()) as usize,
    pad => pad as usize
  };

  pages.iter()
    .rev()
    .enumerate()
    .try_for_each(|(pp, page)|{
      let folio = format!("{:0width$}", pp+1, width=padding);
      let filename = pattern.replace("{}", folio.as_str());
      page.write(&filename, &format, quality)
    })
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

pub fn toBuffer(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  // let this = cx.argument::<BoxedCanvas>(0)?;
  let file_format = string_arg(&mut cx, 1, "format")?;
  let quality = float_arg(&mut cx, 2, "quality")?;
  let callback = cx.argument::<JsFunction>(3)?.root(&mut cx);
  let pages = pages_arg(&mut cx, 4)?;
  let queue = cx.queue();

  std::thread::spawn(move || {

    let encoded = {
      if file_format=="pdf" && pages.len() > 1 {
        to_pdf(&pages)
      }else{
        pages[0].encoded_as(&file_format, quality)
      }
    };

    queue.send(move |mut cx| {
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
  let file_format = string_arg(&mut cx, 1, "format")?;
  let quality = float_arg(&mut cx, 2, "quality")?;
  let pages = pages_arg(&mut cx, 3)?;

    let encoded = {
      if file_format=="pdf" && pages.len() > 1 {
        to_pdf(&pages)
      }else{
        pages[0].encoded_as(&file_format, quality)
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
  let name_pattern = string_arg(&mut cx, 1, "filePath")?;
  let sequence = !cx.argument::<JsValue>(2)?.is_a::<JsUndefined, _>(&mut cx);
  let padding = opt_float_arg(&mut cx, 2).unwrap_or(-1.0);
  let file_format = string_arg(&mut cx, 3, "format")?;
  let quality = float_arg(&mut cx, 4, "quality")?;
  let callback = cx.argument::<JsFunction>(5)?.root(&mut cx);
  let pages = pages_arg(&mut cx, 6)?;
  let queue = cx.queue();

  std::thread::spawn(move || {

    let result = {
      if sequence {
        write_sequence(&pages, &name_pattern, &file_format, padding, quality)
      } else if file_format == "pdf" {
        write_pdf(&name_pattern, &pages)
      } else {
        pages[0].write(&name_pattern, &file_format, quality)
      }
    };

    queue.send(move |mut cx| {
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
  let name_pattern = string_arg(&mut cx, 1, "filePath")?;
  let sequence = !cx.argument::<JsValue>(2)?.is_a::<JsUndefined, _>(&mut cx);
  let padding = opt_float_arg(&mut cx, 2).unwrap_or(-1.0);
  let file_format = string_arg(&mut cx, 3, "format")?;
  let quality = float_arg(&mut cx, 4, "quality")?;
  let pages = pages_arg(&mut cx, 5)?;

  let result = {
    if sequence {
      write_sequence(&pages, &name_pattern, &file_format, padding, quality)
    } else if file_format == "pdf" {
      write_pdf(&name_pattern, &pages)
    } else {
      pages[0].write(&name_pattern, &file_format, quality)
    }
  };

  match result{
    Ok(_) => Ok(cx.undefined()),
    Err(msg) => cx.throw_error(msg)
  }
}
