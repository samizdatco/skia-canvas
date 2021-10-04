#![allow(unused_mut)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(non_snake_case)]
#![allow(dead_code)]
use std::cell::RefCell;
use std::sync::{Arc, Mutex};
use neon::prelude::*;
use skia_safe::{Shader, TileMode, TileMode::{Decal, Repeat}, SamplingOptions, Size,
                Image as SkImage, Picture, Matrix, FilterMode};

use crate::utils::*;
use crate::image::{BoxedImage};
use crate::context::{BoxedContext2D};

pub type BoxedCanvasPattern = JsBox<RefCell<CanvasPattern>>;
impl Finalize for CanvasPattern {}


pub struct Stamp{
  image:Option<SkImage>,
  pict:Option<Picture>,
  dims:Size,
  repeat:(TileMode, TileMode),
  matrix:Matrix
}

#[derive(Clone)]
pub struct CanvasPattern{
  pub stamp:Arc<Mutex<Stamp>>
}

impl CanvasPattern{
  pub fn shader(&self, smoothing: bool) -> Option<Shader>{
    let stamp = Arc::clone(&self.stamp);
    let stamp = stamp.lock().unwrap();

    if let Some(image) = &stamp.image{
      let quality = match smoothing{
        true => FilterQuality::High,
        false => FilterQuality::None
      };

      match image.to_shader(stamp.repeat, to_sampling_opts(quality), None){
        Some(shader) => Some(shader.with_local_matrix(&stamp.matrix)),
        None => None
      }
    }else if let Some(pict) = &stamp.pict{
      let shader = pict.to_shader(stamp.repeat, FilterMode::Linear, None, None);
      Some(shader.with_local_matrix(&stamp.matrix))
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
    let dims = src.size();
    let stamp = Stamp{
      image:src.image.clone(),
      pict:None,
      dims,
      repeat,
      matrix:Matrix::new_identity()
    };
    let stamp = Arc::new(Mutex::new(stamp));
    Ok(cx.boxed(RefCell::new(CanvasPattern{stamp})))
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

    let dims = ctx.bounds.size();
    let stamp = Stamp{
      image:None,
      pict:ctx.get_picture(None),
      dims,
      repeat,
      matrix:Matrix::new_identity()
    };
    let stamp = Arc::new(Mutex::new(stamp));
    Ok(cx.boxed(RefCell::new(CanvasPattern{stamp})))
  }else{
    cx.throw_error("Unknown pattern repeat style")
  }
}

pub fn setTransform(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedCanvasPattern>(0)?;
  let matrix = matrix_arg(&mut cx, 1)?;
  let mut this = this.borrow_mut();
  let stamp = Arc::clone(&this.stamp);
  let mut stamp = stamp.lock().unwrap();

  stamp.matrix = matrix;
  Ok(cx.undefined())
}

pub fn repr(mut cx: FunctionContext) -> JsResult<JsString> {
  let this = cx.argument::<BoxedCanvasPattern>(0)?;
  let mut this = this.borrow_mut();

  let stamp = Arc::clone(&this.stamp);
  let stamp = stamp.lock().unwrap();
  let style = if stamp.image.is_some(){ "Bitmap" }else{ "Canvas" };
  Ok(cx.string(format!("{} {}×{}", style, stamp.dims.width, stamp.dims.height)))
}