#![allow(unused_mut)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]
use neon::prelude::*;
use skia_safe::{Bitmap};

use crate::utils::*;

pub struct Image{
  src:String,
  bitmap:Bitmap
}

declare_types! {
  pub class JsImage for Image {
    init(_) {
      Ok(Image{ src:"".to_string(), bitmap:Bitmap::new() })
    }

    constructor(mut cx){
      Ok(None)
    }

    method get_src(mut cx){
      let mut this = cx.this();
      Ok(cx.undefined().upcast())
    }

    method set_src(mut cx){
      let mut this = cx.this();
      Ok(cx.undefined().upcast())
    }
  }
}
