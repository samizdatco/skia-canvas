#![allow(unused_mut)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]
#![allow(non_snake_case)]
#![allow(clippy::needless_range_loop)]
use std::fs;
use std::path::Path;
use std::cell::RefCell;
use neon::prelude::*;
use neon::result::Throw;
use neon::object::This;
use skia_safe::{Surface, Rect, Picture, EncodedImageFormat, Data, pdf, svg};


use crate::utils::*;
use crate::context::{BoxedContext2D, Context2D};

pub type BoxedCanvas = JsBox<RefCell<Canvas>>;
impl Finalize for Canvas {}

pub struct Canvas{
  pub width: f32,
  pub height: f32,
}

impl Canvas{
  pub fn new(width:f32, height:f32) -> Self{
    let width = if width < 0.0 { 300.0 } else { width };
    let height = if height < 0.0 { 150.0 } else { height };
    Canvas{width, height}
  }

  fn encode_image(&self, picture: &Picture, format:&str, width: f32, height: f32, quality: f32) -> Option<Data> {
    let img_format = match format {
      "jpg" | "jpeg" => Some(EncodedImageFormat::JPEG),
      "png" => Some(EncodedImageFormat::PNG),
      "webp" => Some(EncodedImageFormat::WEBP),
      "gif" => Some(EncodedImageFormat::GIF),
      "heic" => Some(EncodedImageFormat::HEIF),
      _ => None
    };

    if let Some(format) = img_format{
      let img_dims = (width as i32, height as i32);
      if let Some(mut surface) = Surface::new_raster_n32_premul(img_dims){
        surface.canvas().draw_picture(&picture, None, None);
        let img = surface.image_snapshot();
        img.encode_to_data_with_quality(format, quality as i32)
      }else{
        None
      }
    }else if format == "pdf"{
      let img_dims = (width as i32, height as i32);
      let mut document = pdf::new_document(None).begin_page(img_dims, None);
      let canvas = document.canvas();
      canvas.draw_picture(&picture, None, None);
      Some(document.end_page().close())
    }else if format == "svg"{
      let img_dims = (self.width as i32, self.height as i32);
      let mut canvas = svg::Canvas::new(Rect::from_size(img_dims), None);
      canvas.draw_picture(&picture, None, None);
      Some(canvas.end())
    }else{
      None
    }
  }

  fn write_page(&self, page: &mut Context2D, filename: &str, file_format:&str, quality: f32) -> Result<(), String> {
    let path = Path::new(&filename);
    if page.width() == 0.0 || page.height() == 0.0 {
      return Err("Width and height must be non-zero to generate an image".to_string())
    }

    let data = match page.get_picture(None) {
      Some(picture) => self.encode_image(&picture, &file_format, page.width(), page.height(), quality),
      None => None
    };

    match data {
      Some(data) => fs::write(path, data.as_bytes()).map_err(|why|
        format!("{}: \"{}\"", why, path.display())
      ),
      None => Err(format!("Unsupported file format: {:?}", file_format))
    }
  }
}


//
// -- Javascript Methods --------------------------------------------------------------------------
//

pub fn new(mut cx: FunctionContext) -> JsResult<BoxedCanvas> {
  let width = float_arg_or(&mut cx, 1, 300.0).floor();
  let height = float_arg_or(&mut cx, 2, 150.0).floor();
  let this = RefCell::new(Canvas::new(width, height));
  Ok(cx.boxed(this))
}

pub fn get_width(mut cx: FunctionContext) -> JsResult<JsNumber> {
  let this = cx.argument::<BoxedCanvas>(0)?;
  let this = this.borrow();

  Ok(cx.number(this.width as f64))
}

pub fn get_height(mut cx: FunctionContext) -> JsResult<JsNumber> {
  let this = cx.argument::<BoxedCanvas>(0)?;
  let this = this.borrow();

  Ok(cx.number(this.height as f64))
}

pub fn set_width(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedCanvas>(0)?;

  let width = float_arg(&mut cx, 1, "size")?.floor();
  if width >= 0.0 {
    let mut that = this.borrow_mut();
    that.width = width;
  }

  Ok(cx.undefined())
}

pub fn set_height(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedCanvas>(0)?;

  let height = float_arg(&mut cx, 1, "size")?.floor();
  if height >= 0.0 {
    let mut that = this.borrow_mut();
    that.height = height;
  }

  Ok(cx.undefined())
}

pub fn saveAs(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedCanvas>(0)?;
  let name_pattern = string_arg(&mut cx, 1, "filePath")?;
  let sequence = !cx.argument::<JsValue>(2)?.is_a::<JsUndefined, _>(&mut cx);
  let file_format = string_arg(&mut cx, 3, "format")?;
  let quality = float_arg(&mut cx, 4, "quality")?;
  let mut pages = cx.argument::<JsArray>(5)?
          .to_vec(&mut cx)?
          .iter()
          .map(|obj| obj.downcast::<BoxedContext2D, _>(&mut cx))
          .filter( |ctx| ctx.is_ok() )
          .map(|obj| obj.unwrap())
          .collect::<Vec<Handle<BoxedContext2D>>>();

  if sequence {
    pages.reverse();

    let padding = float_arg(&mut cx, 1, "padding")? as i32;
    let padding = match padding {
      -1 => (1.0 + (pages.len() as f32).log10().floor()) as usize,
      _ => padding as usize
    };

    for pp in 0..pages.len() {
      let mut page = &mut pages[pp];
      let filename = name_pattern.replace("{}", format!("{:0width$}", pp+1, width=padding).as_str());
      let this = this.borrow();
      let mut page = page.borrow_mut();
      if let Err(why) = this.write_page(&mut page, &filename, &file_format, quality) {
        return cx.throw_error(why)
      }
    }
  } else if file_format == "pdf" {
    pages.reverse();

    let document = pages.iter_mut().fold(pdf::new_document(None), |doc, page|{
      let mut page = page.borrow_mut();
      let dims = (page.width() as i32, page.height() as i32);
      let mut doc = doc.begin_page(dims, None);
      let canvas = doc.canvas();
      if let Some(picture) = page.get_picture(None){
        canvas.draw_picture(&picture, None, None);
      }
      doc.end_page()
    });

    let path = Path::new(&name_pattern);
    return match fs::write(path, document.close().as_bytes()){
      Err(why) => cx.throw_error(format!("{}: \"{}\"", why, path.display())),
      Ok(()) => Ok(cx.undefined())
    }
  } else {
    let mut page = pages[0];
    let this = this.borrow();
    let mut page = page.borrow_mut();

    if let Err(why) = this.write_page(&mut page, &name_pattern, &file_format, quality) {
      return cx.throw_error(why)
    }
  }

  Ok(cx.undefined())
}

pub fn toBuffer(mut cx: FunctionContext) -> JsResult<JsBuffer> {
  let this = cx.argument::<BoxedCanvas>(0)?;
  let file_format = string_arg(&mut cx, 1, "format")?;
  let quality = float_arg(&mut cx, 2, "quality")?;
  let page_idx = opt_float_arg(&mut cx, 3);
  let mut pages = cx.argument::<JsArray>(4)?
          .to_vec(&mut cx)?
          .iter()
          .map(|obj| obj.downcast::<BoxedContext2D, _>(&mut cx))
          .filter( |ctx| ctx.is_ok() )
          .map(|obj| obj.unwrap())
          .collect::<Vec<Handle<BoxedContext2D>>>();

  let data = {
    if file_format=="pdf" && page_idx.is_none() {
      Some(pages.iter_mut().rev().fold(pdf::new_document(None), |doc, page|{
        let mut page = page.borrow_mut();
        let dims = (page.width() as i32, page.height() as i32);
        let mut doc = doc.begin_page(dims, None);
        let canvas = doc.canvas();
        if let Some(picture) = page.get_picture(None){
          canvas.draw_picture(&picture, None, None);
        }
        doc.end_page()
      }).close())
    }else{
      let page_idx = page_idx.unwrap_or(0.0);
      let this = this.borrow();
      let mut page = pages[page_idx as usize].borrow_mut();
      match page.get_picture(None) {
        Some(picture) => this.encode_image(&picture, &file_format, page.width(), page.height(), quality),
        None => None
      }
    }
  };

  match data{
    Some(data) => {
      let mut buffer = JsBuffer::new(&mut cx, data.len() as u32)?;
      cx.borrow_mut(&mut buffer, |buf_data| {
        buf_data.as_mut_slice().copy_from_slice(&data);
      });
      Ok(buffer)
    },
    None => cx.throw_error(format!("Unsupported image format: {:?}", file_format))
  }

}
