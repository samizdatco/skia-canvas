#![allow(unused_mut)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(non_snake_case)]
#![allow(dead_code)]
use std::cell::RefCell;
use neon::prelude::*;
use skia_safe::{Shader, TileMode, TileMode::{Decal, Repeat}, SamplingOptions,
                Image as SkImage, Picture, Matrix, FilterQuality, FilterMode};

use crate::utils::*;
use crate::image::{BoxedImage};
use crate::context::{BoxedContext2D};

pub type BoxedCanvasPattern = JsBox<RefCell<CanvasPattern>>;
impl Finalize for CanvasPattern {}

#[derive(Clone)]
pub struct CanvasPattern{
  pub smoothing:bool,
  image:Option<SkImage>,
  pict:Option<Picture>,
  repeat:(TileMode, TileMode),
  matrix:Matrix
}

impl CanvasPattern{
  pub fn shader(&self) -> Option<Shader>{
    if let Some(image) = &self.image{
      let sampling = match self.smoothing{
        true => SamplingOptions::from_filter_quality(FilterQuality::High, None),
        false => SamplingOptions::default()
      };
      match image.to_shader(self.repeat, sampling, None){
        Some(shader) => Some(shader.with_local_matrix(&self.matrix)),
        None => None
      }
    }else if let Some(pict) = &self.pict{
      let shader = pict.to_shader(self.repeat, FilterMode::Linear, None, None);
      Some(shader.with_local_matrix(&self.matrix))
    }else{
      None
    }
  }
}

//
// -- Javascript Methods --------------------------------------------------------------------------
//

pub fn from_image(mut cx: FunctionContext) -> JsResult<BoxedCanvasPattern> {
  let src = cx.argument::<BoxedImage>(1)?;
  let repetition = if cx.len() > 2 && cx.argument::<JsValue>(2)?.is_a::<JsNull, _>(&mut cx){
    "".to_string() // null is a valid synonym for "repeat" (as is "")
  }else{
    string_arg(&mut cx, 2, "repetition")?
  };

  if let Some(repeat) = to_repeat_mode(&repetition){
    let src = src.borrow();
    let pattern = CanvasPattern{
      image:src.image.clone(),
      pict:None,
      repeat,
      smoothing:true,
      matrix:Matrix::new_identity()
    };
    Ok(cx.boxed(RefCell::new(pattern)))
  }else{
    cx.throw_error("Unknown pattern repeat style")
  }
}

pub fn from_canvas(mut cx: FunctionContext) -> JsResult<BoxedCanvasPattern> {
  let src = cx.argument::<BoxedContext2D>(1)?;
  let repetition = if cx.len() > 2 && cx.argument::<JsValue>(2)?.is_a::<JsNull, _>(&mut cx){
    "".to_string() // null is a valid synonym for "repeat" (as is "")
  }else{
    string_arg(&mut cx, 2, "repetition")?
  };

  if let Some(repeat) = to_repeat_mode(&repetition){
    let mut ctx = src.borrow_mut();
    let pattern = CanvasPattern{
      image:None,
      pict:ctx.get_picture(None),
      repeat,
      smoothing:true,
      matrix:Matrix::new_identity()
    };
    Ok(cx.boxed(RefCell::new(pattern)))
  }else{
    cx.throw_error("Unknown pattern repeat style")
  }
}

pub fn setTransform(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedCanvasPattern>(0)?;
  let mut this = this.borrow_mut();
  this.matrix = matrix_arg(&mut cx, 1)?;
  Ok(cx.undefined())
}