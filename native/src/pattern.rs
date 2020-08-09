#![allow(unused_imports)]
use std::rc::Rc;
use std::cell::RefCell;
use neon::prelude::*;
use skia_safe::{shaders, Shader, Matrix, TileMode};

use crate::utils::*;
use crate::image::{Image, JsImage};

#[derive(Clone)]
pub struct CanvasPattern{
  shader:Option<Shader>
}

impl CanvasPattern{
  pub fn shader(&self) -> Option<Shader>{
    match &self.shader {
      Some(shader) => Some(shader.clone()),
      None => None
    }
  }
}

use TileMode::*;

declare_types! {
  pub class JsCanvasPattern for CanvasPattern {
    init(_) {
      Ok(CanvasPattern{ shader:None })
    }

    constructor(mut cx){
      let mut this = cx.this();

      let img = cx.argument::<JsImage>(0)?;
      let (tx, ty) = match string_arg(&mut cx, 1, "repetition")?.as_str() {
        "repeat" => (Repeat, Repeat),
        "repeat-x" => (Repeat, Decal),
        "repeat-y" => (Decal, Repeat),
        _ => return cx.throw_error("The string did not match the expected pattern")
      };

      let shader = cx.borrow(&img, |img| {
        match &img.image {
          Some(image) => Some(image.to_shader((tx, ty), None)),
          None => None
        }
      });

      cx.borrow_mut(&mut this, |mut this| this.shader = shader );
      Ok(None)
    }

    method _setTransform(mut cx){
      let mut this = cx.this();
      let matrix = matrix_arg(&mut cx, 0)?;
      cx.borrow_mut(&mut this, |mut this| {
        if let Some(shader) = &this.shader{
          this.shader = Some(shader.with_local_matrix(&matrix));
        }
      });
      Ok(cx.undefined().upcast())
    }

  }
}