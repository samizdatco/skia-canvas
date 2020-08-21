#![allow(unused_imports)]
use std::rc::Rc;
use std::cell::RefCell;
use neon::prelude::*;
use skia_safe::{shaders, Shader, Matrix, TileMode::{Decal, Repeat}};

use crate::utils::*;
use crate::image::{Image, JsImage};

#[derive(Clone)]
pub struct CanvasPattern{
  stamp: Shader,
  shader: Rc<RefCell<Shader>>
}

impl CanvasPattern{
  pub fn shader(&self) -> Option<Shader>{
    Some(self.shader.borrow().clone())
  }
}

declare_types! {
  pub class JsCanvasPattern for CanvasPattern {
    init(_) {
      Ok(CanvasPattern{
        stamp: shaders::empty(),
        shader: Rc::new(RefCell::new(shaders::empty()))
      })
    }

    constructor(mut cx){
      let mut this = cx.this();
      let img = cx.argument::<JsImage>(0)?;
      let repetition = if cx.len() > 1 && cx.argument::<JsValue>(1)?.is_a::<JsNull>(){
        "".to_string() // null is a valid synonym for "repeat" (as is "")
      }else{
        string_arg(&mut cx, 1, "repetition")?
      };

      let (tile_x, tile_y) = match repetition.as_str() {
        "repeat" | "" => (Repeat, Repeat),
        "repeat-x" => (Repeat, Decal),
        "repeat-y" => (Decal, Repeat),
        "no-repeat" => (Decal, Decal),
        _ => return cx.throw_error("Unknown pattern repeat style")
      };

      cx.borrow_mut(&mut this, |mut this|{
        cx.borrow(&img, |img| {
          if let Some(image) = &img.image {
            this.stamp = image.to_shader((tile_x, tile_y), None);
            this.shader.replace(this.stamp.clone());
          }
        });
      });

      Ok(None)
    }

    method _setTransform(mut cx){
      let this = cx.this();
      let matrix = matrix_arg(&mut cx, 0)?;
      cx.borrow(&this, |this| {
        this.shader.replace(this.stamp.with_local_matrix(&matrix));
      });
      Ok(cx.undefined().upcast())
    }

  }
}