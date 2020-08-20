#![allow(unused_mut)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]
#![allow(clippy::needless_range_loop)]
use std::fs;
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

pub fn canvas_pages<'a, T:This>(cx: &mut CallContext<'a, T>, this: &Handle<JsCanvas>)->Result<Vec<Handle<'a, JsContext2D>>, Throw>{
  let context_map = this
      .get(cx, "constructor")?
      .downcast::<JsFunction>().or_throw(cx)?
      .get(cx, "context")?
      .downcast::<JsObject>().or_throw(cx)?;

  let map_getter = context_map
      .get(cx, "get")?
      .downcast::<JsFunction>().or_throw(cx)?;

  let contexts = map_getter
      .call(cx, context_map, vec![this.upcast::<JsObject>()])?
      .downcast::<JsArray>().or_throw(cx)?
      .to_vec(cx)?
      .iter()
      .map(|obj| obj.downcast::<JsContext2D>())
      .filter( |ctx| ctx.is_ok() )
      .map(|obj| obj.unwrap())
      .collect::<Vec<Handle<JsContext2D>>>();

    Ok(contexts)
}


pub fn canvas_context<T:This, F, U>(cx: &mut CallContext<'_, T>, this: &Handle<JsCanvas>, f:F)->Result<U, Throw> where
  T: This,
  F:FnOnce(&mut Context2D) -> U
{
  let mut contexts = canvas_pages(cx, &this)?;
  cx.borrow_mut(&mut contexts[0], |mut ctx|
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

    method _saveAs(mut cx){
      let this = cx.this();
      let name_pattern = string_arg(&mut cx, 0, "filePath")?;
      let sequence = !cx.argument::<JsValue>(1)?.is_a::<JsUndefined>();
      let file_format = string_arg(&mut cx, 2, "format")?;
      let quality = float_arg(&mut cx, 3, "quality")?;

      if sequence {
        let mut pages = canvas_pages(&mut cx, &this)?;
        pages.reverse();

        let padding = float_arg(&mut cx, 1, "padding")? as i32;
        let padding = match padding {
          -1 => (1.0 + (pages.len() as f32).log10().floor()) as usize,
          _ => padding as usize
        };

        for pp in 0..pages.len() {
          let mut page = &mut pages[pp];
          let filename = name_pattern.replace("{}", format!("{:0width$}", pp+1, width=padding).as_str());
          let io = cx.borrow(&this, |this|
            cx.borrow_mut(&mut page, |mut page|{
              this.write_page(&mut page, &filename, &file_format, quality)
            })
          );

          if let Err(why) = io{
            return cx.throw_error(why)
          }
        }
      } else if file_format == "pdf" {
        let mut pages = canvas_pages(&mut cx, &this)?;
        pages.reverse();

        let document = pages.iter_mut().fold(pdf::new_document(None), |doc, page|{
          cx.borrow_mut(page, |mut page| {
            let dims = (page.width() as i32, page.height() as i32);
            let mut doc = doc.begin_page(dims, None);
            let canvas = doc.canvas();
            if let Some(picture) = page.get_picture(None){
              canvas.draw_picture(&picture, None, None);
            }
            doc.end_page()
          })
        });

        let path = Path::new(&name_pattern);
        return match fs::write(path, document.close().as_bytes()){
          Err(why) => cx.throw_error(format!("{}: \"{}\"", why, path.display())),
          Ok(()) => Ok(cx.undefined().upcast())
        }
      } else {
        let mut page = canvas_pages(&mut cx, &this)?[0];
        let io = cx.borrow(&this, |this|
          cx.borrow_mut(&mut page, |mut page|{
            this.write_page(&mut page, &name_pattern, &file_format, quality)
          })
        );

        if let Err(why) = io{
          return cx.throw_error(why)
        }
      }

      Ok(cx.undefined().upcast())
    }

    method _toBuffer(mut cx){
      let this = cx.this();
      let file_format = string_arg(&mut cx, 0, "format")?;
      let quality = float_arg(&mut cx, 1, "quality")?;
      let mut page = canvas_pages(&mut cx, &this)?[0];

      let data = cx.borrow(&this, |this|
        cx.borrow_mut(&mut page, |mut page|{
          match page.get_picture(None) {
            Some(picture) => this.encode_image(&picture, &file_format, page.width(), page.height(), quality),
            None => None
          }
        })
      );

      match data{
        Some(data) => {
          let mut buffer = JsBuffer::new(&mut cx, data.len() as u32)?;
          cx.borrow_mut(&mut buffer, |buf_data| {
            buf_data.as_mut_slice().copy_from_slice(&data);
          });
          Ok(buffer.upcast())
        },
        None => cx.throw_error(format!("Unsupported image format: {:?}", file_format))
      }
    }

  }
}