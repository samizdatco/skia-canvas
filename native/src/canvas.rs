#![allow(unused_mut)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]
use std::fs;
use std::ffi::OsStr;
use std::path::Path;
use neon::prelude::*;
use neon::result::Throw;
use neon::object::This;
use skia_safe::{Surface, Rect, Picture, EncodedImageFormat, Data, pdf, svg};

use crate::utils::*;
use crate::context::{JsContext2D, Context2D};

pub struct Canvas{
  pub width: f32,
  pub height: f32,
  pub density: f32,
}

impl Canvas{
  fn encode_image(&self, picture: &Picture, format:&str) -> Option<Data> {
    let img_format = match format {
      "jpg" | "jpeg" => Some(EncodedImageFormat::JPEG),
      "png" => Some(EncodedImageFormat::PNG),
      "webp" => Some(EncodedImageFormat::WEBP),
      "gif" => Some(EncodedImageFormat::GIF),
      "heic" => Some(EncodedImageFormat::HEIF),
      _ => None
    };

    if let Some(format) = img_format{
      let img_dims = (self.width as i32, self.height as i32);
      if let Some(mut surface) = Surface::new_raster_n32_premul(img_dims){
        surface.canvas().draw_picture(&picture, None, None);
        let img = surface.image_snapshot();
        img.encode_to_data_with_quality(format, 100 /* quality */)
      }else{
        None
      }
    }else if format == "pdf"{
      let img_dims = (self.width as i32, self.height as i32);
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
}

pub fn canvas_context<T:This, F, U>(cx: &mut CallContext<'_, T>, this: &Handle<JsCanvas>, f:F)->Result<U, Throw> where
  T: This,
  F:FnOnce(&mut Context2D) -> U
{
  let context_map = this
      .get(cx, "constructor")?
      .downcast::<JsFunction>().or_throw(cx)?
      .get(cx, "context")?
      .downcast::<JsObject>().or_throw(cx)?;

  let map_getter = context_map
      .get(cx, "get")?
      .downcast::<JsFunction>().or_throw(cx)?;

  let mut context = map_getter
      .call(cx, context_map, vec![this.upcast::<JsObject>()])?
      .downcast::<JsContext2D>().or_throw(cx)?;

  cx.borrow_mut(&mut context, |mut ctx|
    Ok(f(&mut ctx))
  )
}

declare_types! {
  pub class JsCanvas for Canvas {
    init(mut cx) {
      let width = float_arg_or(&mut cx, 0, 300.0).floor();
      let height = float_arg_or(&mut cx, 1, 150.0).floor();
      let density = float_arg_or(&mut cx, 2, 1.0).max(1.0);

      let width = if width < 0.0 { 300.0 } else { width };
      let height = if height < 0.0 { 150.0 } else { height };

      Ok(Canvas{ width, height, density })
    }

    method get_density(mut cx){
      let this = cx.this();
      let size = cx.borrow(&this, |this| this.density );
      Ok(cx.number(size).upcast())
    }

    method set_width(mut cx){
      let mut this = cx.this();
      let width = float_arg(&mut cx, 0, "size")?.floor();
      if width >= 0.0 {
        let dims = cx.borrow_mut(&mut this, |mut this| {
          this.width = width;
          (this.width, this.height)
        });

        canvas_context(&mut cx, &this, |ctx|{
          ctx.resize(dims)
        })?;
      }
      Ok(cx.undefined().upcast())
    }

    method get_width(mut cx){
      let this = cx.this();
      let size = cx.borrow(&this, |this| this.width );
      Ok(cx.number(size).upcast())
    }

    method set_height(mut cx){
      let mut this = cx.this();
      let height = float_arg(&mut cx, 0, "size")?.floor();
      if height >= 0.0 {
        let dims = cx.borrow_mut(&mut this, |mut this| {
          this.height = height;
          (this.width, this.height)
        });

        canvas_context(&mut cx, &this, |ctx|{
          ctx.resize(dims)
        })?;
      }
      Ok(cx.undefined().upcast())
    }

    method get_height(mut cx){
      let this = cx.this();
      let size = cx.borrow(&this, |this| this.height );
      Ok(cx.number(size).upcast())
    }

    //
    // Output
    //

    method saveAs(mut cx){
      let this = cx.this();
      let filename = string_arg(&mut cx, 0, "filePath")?;
      let path = Path::new(&filename);
      let extension = path
          .extension()
          .and_then(OsStr::to_str)
          .unwrap_or_default()
          .to_string()
          .to_lowercase();

      let data = match canvas_context(&mut cx, &this, |ctx| ctx.get_picture() )?{
        Some(pic) => cx.borrow(&this, |this|
          this.encode_image(&pic, &extension)
        ),
        None => None
      };

      match data {
        Some(data) => {
          match fs::write(path, data.as_bytes()){
            Err(why) => cx.throw_error(format!("{}: \"{}\"", why, path.display())),
            Ok(()) => Ok(cx.undefined().upcast())
          }
        },
        None => {
          if cx.borrow(&this, |this| this.width==0.0 || this.height==0.0 ){
            cx.throw_error("Width and height must be non-zero to generate an image")
          }else if extension.is_empty() {
            cx.throw_error("Could not determine format from file name")
          }else{
            cx.throw_error(format!("Unsupported file format: {:?}", extension))
          }
        }
      }
    }

    method toBuffer(mut cx){
      let this = cx.this();
      let extension = string_arg_or(&mut cx, 0, "png");

      match extension.to_lowercase().as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "heic" | "webp" | "svg" | "pdf" => {},
        _ => return cx.throw_error(format!("Unrecognized format: {:?}", extension))
      };

      let data = match canvas_context(&mut cx, &this, |ctx| ctx.get_picture() )?{
        Some(pic) => cx.borrow(&this, |this|
          this.encode_image(&pic, &extension.to_lowercase())
        ),
        None => None
      };

      match data{
        Some(data) => {
          let mut buffer = JsBuffer::new(&mut cx, data.len() as u32)?;
          cx.borrow_mut(&mut buffer, |buf_data| {
            buf_data.as_mut_slice().copy_from_slice(&data);
          });
          Ok(buffer.upcast())
        },
        None => cx.throw_error(format!("Unsupported image format: {:?}", extension))
      }
    }

  }
}