#![allow(unused_mut)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]
use std::rc::Rc;
use std::cell::RefCell;
use neon::prelude::*;
use skia_safe::{Surface, PictureRecorder, EncodedImageFormat};

use crate::utils::*;
use crate::context::JsContext2D;

pub struct Canvas{
  pub surface: Rc<RefCell<Surface>>,
  width: i32,
  height: i32,
  density: f32,
}

impl Canvas{
  fn resize(&mut self){
    let size = (self.width, self.height);
    if let Some(bitmap_surface) = Surface::new_raster_n32_premul(size){
      self.surface.replace(bitmap_surface);
    }
  }
}

declare_types! {
  pub class JsCanvas for Canvas {
    init(mut cx) {
      let width = float_arg_or(&mut cx, 0, 300.0).floor();
      let height = float_arg_or(&mut cx, 1, 150.0).floor();
      let density = float_arg_or(&mut cx, 2, 1.0).max(1.0);

      let width = if width < 0.0 { 300.0 } else { width };
      let height = if height < 0.0 { 150.0 } else { height };
      let dims = ((width * density) as i32, (height * density) as i32);

      if let Some(bitmap_surface) = Surface::new_raster_n32_premul(dims){
        let surface = Rc::new(RefCell::new(bitmap_surface));
        let (width, height) = dims;
        Ok(Canvas{ width, height, density, surface })
      }else{
        cx.throw_error(format!("Could not create a canvas of that size ({}×{})", width, height))
      }
    }

    method set_width(mut cx){
      let mut this = cx.this();
      let width = float_arg(&mut cx, 0, "size")?.floor();
      if width >= 0.0 {
        cx.borrow_mut(&mut this, |mut this| {
          this.width = width as i32;
          this.resize();
        });

        let sym = symbol(&mut cx, "ctx")?;
        let ctx = this.get(&mut cx, sym)?;
        if ctx.is_a::<JsContext2D>(){
          if let Ok(mut ctx) = ctx.downcast::<JsContext2D>(){
            cx.borrow_mut(&mut ctx, |mut ctx| ctx.reset_state() )
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
        cx.borrow_mut(&mut this, |mut this| {
          this.height = height as i32;
          this.resize();
        });

        let sym = symbol(&mut cx, "ctx")?;
        let ctx = this.get(&mut cx, sym)?;
        if ctx.is_a::<JsContext2D>(){
          if let Ok(mut ctx) = ctx.downcast::<JsContext2D>(){
            cx.borrow_mut(&mut ctx, |mut ctx| ctx.reset_state() )
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
        let img = this.surface.borrow_mut().image_snapshot();
        img.encode_to_data(EncodedImageFormat::PNG)
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