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
use crate::image::{BoxedImage, Content};
use crate::context::BoxedContext2D;
use crate::filter::ImageFilter;

pub type BoxedCanvasPattern = JsBox<RefCell<CanvasPattern>>;
impl Finalize for CanvasPattern {}


pub struct Stamp{
  content: Content,
  dims:Size,
  repeat:(TileMode, TileMode),
  matrix:Matrix
}

#[derive(Clone)]
pub struct CanvasPattern{
  pub stamp:Arc<Mutex<Stamp>>
}

impl CanvasPattern{
  pub fn shader(&self, image_filter: ImageFilter) -> Option<Shader>{
    let stamp = Arc::clone(&self.stamp);
    let stamp = stamp.lock().unwrap();

    match &stamp.content{
      Content::Bitmap(image) =>
        image.to_shader(stamp.repeat, image_filter.sampling(), None).map(|shader|
          shader.with_local_matrix(&stamp.matrix)
        ),
      Content::Vector(pict) => {
        let shader = pict.to_shader(stamp.repeat, FilterMode::Linear, None, None);
        Some(shader.with_local_matrix(&stamp.matrix))
      },
      _ => None
    }
  }
}

//
// -- Javascript Methods --------------------------------------------------------------------------
//

pub fn from_image(mut cx: FunctionContext) -> JsResult<BoxedCanvasPattern> {
  let src = cx.argument::<BoxedImage>(1)?;
  let canvas_width = float_arg(&mut cx, 2, "width")?;
  let canvas_height = float_arg(&mut cx, 3, "height")?;
  let repetition = if cx.len() > 4 && cx.argument::<JsValue>(4)?.is_a::<JsNull, _>(&mut cx){
    "".to_string() // null is a valid synonym for "repeat" (as is "")
  }else{
    string_arg(&mut cx, 4, "repetition")?
  };

  if let Some(repeat) = to_repeat_mode(&repetition){
    let src = src.borrow();
    let dims:Size = src.content.size().into();
    let mut matrix = Matrix::new_identity();

    if src.autosized && !dims.is_empty() {
      // If this flag is set (for SVG images with no intrinsic size) then we need to scale the image to
      // the canvas' smallest dimension. This preserves compatibility with how Chromium browsers behave.
      let min_size = f32::min(canvas_width, canvas_height);
      let factor = (min_size / dims.width, min_size / dims.height);
      matrix.set_scale(factor, None);
    }

    let content = src.content.clone();
    let stamp = Arc::new(Mutex::new(Stamp{
      content, dims, repeat, matrix
    }));
    Ok(cx.boxed(RefCell::new(CanvasPattern{stamp})))
  }else{
    cx.throw_error("Unknown pattern repeat style")
  }
}

pub fn from_image_data(mut cx: FunctionContext) -> JsResult<BoxedCanvasPattern> {
  let src = image_data_arg(&mut cx, 1)?;
  let repetition = if cx.len() > 2 && cx.argument::<JsValue>(2)?.is_a::<JsNull, _>(&mut cx){
    "".to_string() // null is a valid synonym for "repeat" (as is "")
  }else{
    string_arg(&mut cx, 2, "repetition")?
  };

  if let Some(repeat) = to_repeat_mode(&repetition){
    let content = Content::from_image_data(src);
    let dims:Size = content.size().into();
    let mut matrix = Matrix::new_identity();
    let stamp = Arc::new(Mutex::new(Stamp{
      content, dims, repeat, matrix
    }));
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

    let content = ctx.get_picture()
      .map(|picture| Content::Vector(picture))
      .unwrap_or_default();
    let dims = ctx.bounds.size();
    let stamp = Stamp{
      content,
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
  let style = match stamp.content{
    Content::Bitmap(..) => "Bitmap",
    _ => "Canvas"
  };

  Ok(cx.string(format!("{} {}×{}", style, stamp.dims.width, stamp.dims.height)))
}