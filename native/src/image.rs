#![allow(unused_mut)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]
use std::rc::Rc;
use std::cell::RefCell;
use neon::prelude::*;
use skia_safe::{Image as SkImage, Data, Bitmap};

use crate::utils::*;

pub struct Image{
  src:String,
  pub image:Option<SkImage>
}

declare_types! {
  pub class JsImage for Image {
    init(_) {
      Ok(Image{ src:"".to_string(), image:None })
    }

    constructor(mut cx){
      Ok(None)
    }

    method set_data(mut cx){
      let mut this = cx.this();
      let buffer = cx.argument::<JsBuffer>(0)?;
      let data = cx.borrow(&buffer, |buf_data| {
        Data::new_copy(buf_data.as_slice())
      });
      let success = cx.borrow_mut(&mut this, |mut this| {
        this.image = SkImage::from_encoded(data, None);
        this.image.is_some()
      });

      Ok(cx.boolean(success).upcast())
    }

    method get_src(mut cx){
      let mut this = cx.this();
      let src = cx.borrow(&this, |this| this.src.clone());
      Ok(cx.string(src).upcast())
    }

    method set_src(mut cx){
      let mut this = cx.this();
      let src = string_arg(&mut cx, 0, "src")?;
      cx.borrow_mut(&mut this, |mut this| this.src = src.clone() );
      Ok(cx.undefined().upcast())
    }

    method get_width(mut cx){
      let this = cx.this();
      let width = cx.borrow(&this, |this| {
        match &this.image {
          Some(image) => Some(image.width() as f64),
          None => None
        }
      });

      match width{
        Some(size) => Ok(cx.number(size).upcast()),
        None => Ok(cx.undefined().upcast())
      }
    }

    method get_height(mut cx){
      let this = cx.this();
      let height = cx.borrow(&this, |this| {
        match &this.image {
          Some(image) => Some(image.height() as f64),
          None => None
        }
      });

      match height{
        Some(size) => Ok(cx.number(size).upcast()),
        None => Ok(cx.undefined().upcast())
      }
    }

    method get_complete(mut cx){
      let this = cx.this();
      let complete = cx.borrow(&this, |this| this.image.is_some() );
      Ok(cx.boolean(complete).upcast())
    }

  }
}
