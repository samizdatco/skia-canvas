#![allow(unused_mut)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]
use std::rc::Rc;
use std::cell::RefCell;
use neon::prelude::*;
use skia_safe::{Surface, Rect, PictureRecorder, EncodedImageFormat};

use crate::utils::*;
use crate::context::JsContext2D;

pub struct Canvas{
  pub recorder: Rc<RefCell<PictureRecorder>>,
  pub width: f32,
  pub height: f32,
  density: f32,
}

declare_types! {
  pub class JsCanvas for Canvas {
    init(mut cx) {
      let width = float_arg_or(&mut cx, 0, 300.0).floor();
      let height = float_arg_or(&mut cx, 1, 150.0).floor();
      let density = float_arg_or(&mut cx, 2, 1.0).max(1.0);

      let width = if width < 0.0 { 300.0 } else { width };
      let height = if height < 0.0 { 150.0 } else { height };
      let bounds = Rect::from_wh(width * density, height * density);

      let recorder = Rc::new(RefCell::new(PictureRecorder::new()));
      recorder.borrow_mut().begin_recording(bounds, None, None);
      Ok(Canvas{ width, height, density, recorder})
    }

    method set_width(mut cx){
      let mut this = cx.this();
      let width = float_arg(&mut cx, 0, "size")?.floor();
      if width >= 0.0 {
        let dims = cx.borrow_mut(&mut this, |mut this| {
          this.width = width;
          (this.width, this.height)
        });

        let sym = symbol(&mut cx, "ctx")?;
        let ctx = this.get(&mut cx, sym)?;
        if ctx.is_a::<JsContext2D>(){
          if let Ok(mut ctx) = ctx.downcast::<JsContext2D>(){
            cx.borrow_mut(&mut ctx, |mut ctx| ctx.resize(dims) )
          }
        }
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

        let sym = symbol(&mut cx, "ctx")?;
        let ctx = this.get(&mut cx, sym)?;
        if ctx.is_a::<JsContext2D>(){
          if let Ok(mut ctx) = ctx.downcast::<JsContext2D>(){
            cx.borrow_mut(&mut ctx, |mut ctx| ctx.resize(dims) )
          }
        }
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

    method toBuffer(mut cx){
      let this = cx.this();

      let raster = cx.borrow(&this, |this|{
        let mut recorder = this.recorder.borrow_mut();
        if let Some(picture) = recorder.finish_recording_as_picture(None){
          let img_dims = (this.width as i32, this.height as i32);
          if let Some(mut bitmap_surface) = Surface::new_raster_n32_premul(img_dims){
            bitmap_surface.canvas().draw_picture(&picture, None, None);
            let img = bitmap_surface.image_snapshot();
            return img.encode_to_data(EncodedImageFormat::PNG)
          }
        }
        None
      });

      if let Some(data) = raster{
        let mut buffer = JsBuffer::new(&mut cx, data.len() as u32)?;
        cx.borrow_mut(&mut buffer, |buf_data| {
          buf_data.as_mut_slice().copy_from_slice(&data);
        });
        Ok(buffer.upcast())
      }else{
        Ok(cx.undefined().upcast())
      }
    }

  }
}