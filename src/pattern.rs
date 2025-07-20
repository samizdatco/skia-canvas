#![allow(non_snake_case)]
use std::cell::RefCell;
use std::rc::Rc;
use neon::prelude::*;
use skia_safe::{Shader, TileMode, Size, Matrix, FilterMode};

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
  pub stamp:Rc<RefCell<Stamp>>
}

impl CanvasPattern{
  pub fn shader(&self, image_filter: ImageFilter) -> Option<Shader>{
    let stamp = self.stamp.borrow();

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

  pub fn is_opaque(&self) -> bool{
    let stamp = self.stamp.borrow();

    match &stamp.content{
      Content::Bitmap(image) => image.is_opaque(),
      _ => false
    }
  }
}

//
// -- Javascript Methods --------------------------------------------------------------------------
//

pub fn from_image(mut cx: FunctionContext) -> JsResult<BoxedCanvasPattern> {
  let src = cx.argument::<BoxedImage>(1)?;
  let canvas_width = float_arg_or_bail(&mut cx, 2, "width")?;
  let canvas_height = float_arg_or_bail(&mut cx, 3, "height")?;
  let repeat = repetition_arg(&mut cx, 4)?;

  let src = src.borrow();
  let content = src.content.clone();
  let dims:Size = src.content.size().into();
  let mut matrix = Matrix::new_identity();

  if src.autosized && !dims.is_empty() {
    // If this flag is set (for SVG images with no intrinsic size) then we need to scale the image to
    // the canvas' smallest dimension. This preserves compatibility with how Chromium browsers behave.
    let min_size = f32::min(canvas_width, canvas_height);
    let factor = (min_size / dims.width, min_size / dims.height);
    matrix.set_scale(factor, None);
  }

  let stamp = Stamp{content, dims, repeat, matrix};
  let canvas_pattern = CanvasPattern{ stamp:Rc::new(RefCell::new(stamp))};
  let this = RefCell::new(canvas_pattern);
  Ok(cx.boxed(this))
}

pub fn from_image_data(mut cx: FunctionContext) -> JsResult<BoxedCanvasPattern> {
  let src = image_data_arg(&mut cx, 1)?;
  let repeat = repetition_arg(&mut cx, 2)?;
  let content = Content::from_image_data(src);
  let dims:Size = content.size().into();
  let matrix = Matrix::new_identity();

  let stamp = Stamp{content, dims, repeat, matrix};
  let canvas_pattern = CanvasPattern{ stamp:Rc::new(RefCell::new(stamp))};
  let this = RefCell::new(canvas_pattern);
  Ok(cx.boxed(this))
}

pub fn from_canvas(mut cx: FunctionContext) -> JsResult<BoxedCanvasPattern> {
  let src = cx.argument::<BoxedContext2D>(1)?;
  let repeat = repetition_arg(&mut cx, 2)?;

  let mut ctx = src.borrow_mut();
  let content = ctx.get_picture()
    .map(|picture| Content::Vector(picture))
    .unwrap_or_default();
  let dims = ctx.bounds.size();
  let matrix = Matrix::new_identity();

  let stamp = Stamp{content, dims, repeat, matrix};
  let canvas_pattern = CanvasPattern{ stamp:Rc::new(RefCell::new(stamp))};
  let this = RefCell::new(canvas_pattern);
  Ok(cx.boxed(this))
}

pub fn setTransform(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedCanvasPattern>(0)?;
  let matrix = matrix_arg(&mut cx, 1)?;
  let this = this.borrow();

  let mut stamp = this.stamp.borrow_mut();
  stamp.matrix = matrix;
  Ok(cx.undefined())
}

pub fn repr(mut cx: FunctionContext) -> JsResult<JsString> {
  let this = cx.argument::<BoxedCanvasPattern>(0)?;
  let this = this.borrow();

  let stamp = this.stamp.borrow();
  let style = match stamp.content{
    Content::Bitmap(..) => "Bitmap",
    _ => "Canvas"
  };

  Ok(cx.string(format!("{} {}×{}", style, stamp.dims.width, stamp.dims.height)))
}