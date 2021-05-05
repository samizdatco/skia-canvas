#![allow(unused_mut)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]
use std::rc::Rc;
use std::cell::RefCell;
use neon::prelude::*;
use skia_safe::{Image as SkImage, ImageInfo, ColorType, AlphaType, Data, Bitmap};

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
        this.image = SkImage::from_encoded(data);
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


pub struct ImageData{
  pub width: f32,
  pub height: f32
}

impl ImageData{
  pub fn get_info(&self) -> ImageInfo {
    let dims = (self.width as i32, self.height as i32);
    ImageInfo::new(dims, ColorType::RGBA8888, AlphaType::Unpremul, None)
  }
}

declare_types! {
  pub class JsImageData for ImageData {
    init(mut cx) {
      let width = float_arg(&mut cx, 0, "width")?;
      let height = float_arg(&mut cx, 1, "height")?;

      if width<=0.0 || height <=0.0{
        return cx.throw_range_error("Cannot allocate a buffer of this size")
      }

      Ok(ImageData{ width, height })
    }

    method get_width(mut cx){
      let this = cx.this();
      let width = cx.borrow(&this, |this| this.width );
      Ok(cx.number(width).upcast())
    }

    method get_height(mut cx){
      let this = cx.this();
      let height = cx.borrow(&this, |this| this.height );
      Ok(cx.number(height).upcast())
    }

    // setters are noops
    method set_width(mut cx){
      let arg = cx.argument::<JsValue>(0)?;
      Ok(arg)
    }

    method set_height(mut cx){
      let arg = cx.argument::<JsValue>(0)?;
      Ok(arg)
    }


  }
}