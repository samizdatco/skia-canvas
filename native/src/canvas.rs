#![allow(unused_mut)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]
use std::rc::Rc;
use std::cell::RefCell;
use neon::prelude::*;
use skia_safe::{Canvas as SkCanvas, Surface, ImageInfo, ColorType, AlphaType, Data,
                Bitmap, PictureRecorder};

use crate::utils::*;
use crate::context::JsContext2D;

pub struct Canvas{
  width: f32,
  height: f32,
  density: f32,
  rec:PictureRecorder
}

declare_types! {
  pub class JsCanvas for Canvas {
    init(mut cx) {
      let width = float_arg_or(&mut cx, 0, 300.0).floor();
      let height = float_arg_or(&mut cx, 1, 150.0).floor();
      let density = float_arg_or(&mut cx, 2, 1.0).max(1.0);

      let width = if width < 0.0 { 300.0 } else { width };
      let height = if height < 0.0 { 150.0 } else { height };
      let rec = PictureRecorder::new();

      Ok(Canvas{ width, height, density, rec })
    }

    method getContext(mut cx){
      let mut this = cx.this();
      let kind = string_arg(&mut cx, 0, "kind")?;
      if kind.as_str() == "2d"{
        Ok(cx.undefined().upcast())
      }else{
        Ok(cx.null().upcast())
      }
    }

    method set_width(mut cx){
      let mut this = cx.this();
      let width = float_arg(&mut cx, 0, "size")?.floor();
      if width >= 0.0 {
        cx.borrow_mut(&mut this, |mut this| this.width = width);
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
        cx.borrow_mut(&mut this, |mut this| this.height = height);
      }
      Ok(cx.undefined().upcast())
    }

    method get_height(mut cx){
      let this = cx.this();
      let size = cx.borrow(&this, |this| this.height );
      Ok(cx.number(size).upcast())
    }

  }
}