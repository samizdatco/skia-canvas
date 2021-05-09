#![allow(unused_variables)]
#![allow(unused_mut)]
#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(non_snake_case)]
use std::f32::consts::PI;
use std::cell::RefCell;
use neon::prelude::*;
use skia_safe::{Path, Matrix, Rect, PathDirection, PaintStyle};
use skia_safe::path::AddPathMode::Append;
use skia_safe::textlayout::{TextDirection};
use skia_safe::PaintStyle::{Fill, Stroke};

use super::{Context2D, BoxedContext2D, Dye};
use crate::canvas::{BoxedCanvas};
use crate::path::{Path2D, BoxedPath2D};
use crate::image::{Image, BoxedImage};
use crate::typography::*;
use crate::utils::*;

//
// The js interface for the Context2D struct
//

pub fn new(mut cx: FunctionContext) -> JsResult<BoxedContext2D> {
  let dims = float_args(&mut cx, 1..3)?;

  let bounds = Rect::from_wh(dims[0], dims[1]);
  let this = RefCell::new(Context2D::new(bounds));
  Ok(cx.boxed(this))
}

pub fn resetWidth(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();

  let new_width = float_arg(&mut cx, 1, "width")?;
  let old_height = this.bounds.size().height;
  this.resize((new_width, old_height));
  Ok(cx.undefined())
}

pub fn resetHeight(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();

  let new_height = float_arg(&mut cx, 1, "height")?;
  let old_width = this.bounds.size().width;
  this.resize((old_width, new_height));
  Ok(cx.undefined())
}

//
// Grid State
//

pub fn save(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  this.push();
  Ok(cx.undefined())
}

pub fn restore(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  this.pop();
  Ok(cx.undefined())
}

pub fn transform(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  let t = float_args(&mut cx, 1..7)?;
  let matrix = Matrix::new_all(t[0], t[2], t[4], t[1], t[3], t[5], 0.0, 0.0, 1.0);

  this.with_matrix(|ctm| ctm.pre_concat(&matrix) );
  Ok(cx.undefined())
}

pub fn translate(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  let dx = float_arg(&mut cx, 1, "deltaX")?;
  let dy = float_arg(&mut cx, 2, "deltaY")?;

  this.with_matrix(|ctm| ctm.pre_translate((dx, dy)) );
  Ok(cx.undefined())
}

pub fn scale(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  let x_scale = float_arg(&mut cx, 1, "xScale")?;
  let y_scale = float_arg(&mut cx, 2, "yScale")?;

  this.with_matrix(|ctm| ctm.pre_scale((x_scale, y_scale), None) );
  Ok(cx.undefined())
}

pub fn rotate(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  let radians = float_arg(&mut cx, 1, "angle")?;
  let degrees = radians / PI * 180.0;

  this.with_matrix(|ctm| ctm.pre_rotate(degrees, None) );
  Ok(cx.undefined())
}

pub fn resetTransform(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();

  this.with_matrix(|ctm| ctm.reset() );
  Ok(cx.undefined())
}

// -- ctm property ----------------------------------------------------------------------

pub fn get_currentTransform(mut cx: FunctionContext) -> JsResult<JsArray> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let this = this.borrow();

  let array = JsArray::new(&mut cx, 9);
  for i in 0..9 {
    let num = cx.number(this.state.matrix[i as usize]);
    array.set(&mut cx, i as u32, num)?;
  }
  Ok(array)
}

pub fn set_currentTransform(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();

  if let Ok(matrix) = matrix_arg(&mut cx, 1){
    this.with_matrix(|ctm| ctm.reset().pre_concat(&matrix) );
  }
  Ok(cx.undefined())
}


//
// Bézier Paths
//

pub fn beginPath(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();

  this.path = Path::new();
  Ok(cx.undefined())
}

// -- primitives ------------------------------------------------------------------------

pub fn rect(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  let nums = float_args(&mut cx, 1..5)?;

  if let [x, y, w, h] = nums.as_slice(){
    let rect = Rect::from_xywh(*x, *y, *w, *h);
    let matrix = this.state.matrix;
    let mut rect_path = Path::new();
    rect_path.add_rect(&rect, Some((PathDirection::CW, 0)));
    this.path.add_path(&rect_path.with_transform(&matrix), (0, 0), Append);
  }
  Ok(cx.undefined())
}

pub fn arc(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  let nums = float_args(&mut cx, 1..6)?;
  let ccw = bool_arg_or(&mut cx, 6, false);

  if let [x, y, radius, start_angle, end_angle] = nums.as_slice(){
    let matrix = this.state.matrix;
    let mut arc = Path2D::new();
    arc.add_ellipse((*x, *y), (*radius, *radius), 0.0, *start_angle, *end_angle, ccw);
    this.path.add_path(&arc.path.with_transform(&matrix), (0,0), Append);
  }
  Ok(cx.undefined())
}

pub fn ellipse(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  let nums = float_args(&mut cx, 1..8)?;
  let ccw = bool_arg_or(&mut cx, 8, false);

  if let [x, y, x_radius, y_radius, rotation, start_angle, end_angle] = nums.as_slice(){
    if *x_radius < 0.0 || *y_radius < 0.0 {
      return cx.throw_error("radii cannot be negative")
    }
    let matrix = this.state.matrix;
    let mut arc = Path2D::new();
    arc.add_ellipse((*x, *y), (*x_radius, *y_radius), *rotation, *start_angle, *end_angle, ccw);
    this.path.add_path(&arc.path.with_transform(&matrix), (0,0), Append);
  }
  Ok(cx.undefined())
}

// contour drawing ----------------------------------------------------------------------

pub fn moveTo(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  let x = float_arg(&mut cx, 1, "x")?;
  let y = float_arg(&mut cx, 2, "y")?;

  if let [dst] = this.map_points(&[x, y])[..1]{
    this.path.move_to(dst);
  }
  Ok(cx.undefined())
}

pub fn lineTo(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  let x = float_arg(&mut cx, 1, "x")?;
  let y = float_arg(&mut cx, 2, "y")?;

  if let [dst] = this.map_points(&[x, y])[..1]{
    if this.path.is_empty(){ this.path.move_to(dst); }
    this.path.line_to(dst);
  }
  Ok(cx.undefined())
}

pub fn arcTo(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  let coords = float_args(&mut cx, 1..5)?;
  let radius = float_arg(&mut cx, 5, "radius")?;

  if let [src, dst] = this.map_points(&coords)[..2]{
    if this.path.is_empty(){ this.path.move_to(src); }
    this.path.arc_to_tangent(src, dst, radius);
  }
  Ok(cx.undefined())
}

pub fn bezierCurveTo(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  let coords = float_args(&mut cx, 1..7)?;
  if let [cp1, cp2, dst] = this.map_points(&coords)[..3]{
    if this.path.is_empty(){ this.path.move_to(cp1); }
    this.path.cubic_to(cp1, cp2, dst);
  }
  Ok(cx.undefined())
}

pub fn quadraticCurveTo(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  let coords = float_args(&mut cx, 1..5)?;

  if let [cp, dst] = this.map_points(&coords)[..2]{
    if this.path.is_empty(){ this.path.move_to(cp); }
    this.path.quad_to(cp, dst);
  }
  Ok(cx.undefined())
}

pub fn closePath(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();

  this.path.close();
  Ok(cx.undefined())
}

// hit testing --------------------------------------------------------------------------

pub fn isPointInPath(mut cx: FunctionContext) -> JsResult<JsBoolean> {
  _is_in(cx, Fill)
}

pub fn isPointInStroke(mut cx: FunctionContext) -> JsResult<JsBoolean> {
  _is_in(cx, Stroke)
}

fn _is_in(mut cx: FunctionContext, ink:PaintStyle) -> JsResult<JsBoolean> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let (mut container, shift) = match cx.argument::<JsValue>(1)?.is_a::<BoxedPath2D, _>(&mut cx){
    true => (cx.argument::<BoxedContext2D>(1)?, 2),
    false => (this, 1)
  };
  let x = float_arg(&mut cx, shift, "x")?;
  let y = float_arg(&mut cx, shift+1, "y")?;
  let rule = fill_rule_arg_or(&mut cx, shift+2, "nonzero")?;

  let mut target = container.borrow_mut().path.clone();
  let mut this = this.borrow_mut();
  let is_in = match ink{
    Stroke => this.hit_test_path(&mut target, (x, y), None, Stroke),
    _ => this.hit_test_path(&mut target, (x, y), Some(rule), Fill)
  };
  Ok(cx.boolean(is_in))
}

// masking ------------------------------------------------------------------------------

pub fn clip(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();

  let mut shift = 1;
  let clip = path2d_arg_opt(&mut cx, 1);
  if clip.is_some() { shift += 1; }

  let rule = fill_rule_arg_or(&mut cx, shift, "nonzero")?;
  this.clip_path(clip, rule);

  Ok(cx.undefined())
}


//
// Fill & Stroke
//

pub fn fill(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  let mut shift = 1;
  if let Some(path) = path2d_arg_opt(&mut cx, shift){
    this.path = path.with_transform(&this.state.matrix);
    shift += 1;
  }
  let rule = fill_rule_arg_or(&mut cx, shift, "nonzero")?;

  let paint = this.paint_for_fill();
  this.path.set_fill_type(rule);
  this.draw_path(&paint);
  Ok(cx.undefined())
}

pub fn stroke(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  if let Some(path) = path2d_arg_opt(&mut cx, 1){
    this.path = path.with_transform(&this.state.matrix)
  }

  let paint = this.paint_for_stroke();
  this.draw_path(&paint);
  Ok(cx.undefined())
}

pub fn fillRect(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  let nums = float_args(&mut cx, 1..5)?;
  if let [x, y, w, h] = nums.as_slice() {
    let rect = Rect::from_xywh(*x, *y, *w, *h);
    let paint =  this.paint_for_fill();
    this.draw_rect(&rect, &paint);
  }
  Ok(cx.undefined())
}

pub fn strokeRect(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  let nums = float_args(&mut cx, 1..5)?;
  if let [x, y, w, h] = nums.as_slice() {
    let rect = Rect::from_xywh(*x, *y, *w, *h);
    let paint =  this.paint_for_stroke();
    this.draw_rect(&rect, &paint);
  }
  Ok(cx.undefined())
}

pub fn clearRect(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  let nums = float_args(&mut cx, 1..5)?;
  if let [x, y, w, h] = nums.as_slice() {
    let rect = Rect::from_xywh(*x, *y, *w, *h);
    this.clear_rect(&rect);
  }
  Ok(cx.undefined())
}


// fill & stoke properties --------------------------------------------------------------

pub fn get_fillStyle(mut cx: FunctionContext) -> JsResult<JsValue> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let this = this.borrow();
  let dye = this.state.fill_style.clone();
  dye.value(&mut cx, Fill)
}

pub fn set_fillStyle(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  let arg = cx.argument::<JsValue>(1)?;

  if let Some(dye) = Dye::new(&mut cx, arg, Fill) {
    this.state.fill_style = dye;
  }else{
    eprintln!("Warning: Invalid fill style (expected a css color string, CanvasGradient, or CanvasPattern)");
  }
  Ok(cx.undefined())
}

pub fn get_strokeStyle(mut cx: FunctionContext) -> JsResult<JsValue> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let this = this.borrow();
  let dye = this.state.stroke_style.clone();
  dye.value(&mut cx, Stroke)
}

pub fn set_strokeStyle(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  let arg = cx.argument::<JsValue>(1)?;

  if let Some(dye) = Dye::new(&mut cx, arg, Stroke) {
    this.state.stroke_style = dye;
  }else{
    eprintln!("Warning: Invalid stroke style (expected a css color string, CanvasGradient, or CanvasPattern)");
  }
  Ok(cx.undefined())
}

//
// Line Style
//

pub fn getLineDash(mut cx: FunctionContext) -> JsResult<JsValue> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  let dashes = this.state.line_dash_list.clone();
  floats_to_array(&mut cx, &dashes)
}

pub fn setLineDash(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  let arg = cx.argument::<JsValue>(1)?;
  if arg.is_a::<JsArray, _>(&mut cx) {
    let list = cx.argument::<JsArray>(1)?.to_vec(&mut cx)?;
    let mut intervals = floats_in(&mut cx, &list).iter().cloned()
      .filter(|n| *n >= 0.0)
      .collect::<Vec<f32>>();

    if list.len() == intervals.len(){
      if intervals.len() % 2 == 1{
        intervals.append(&mut intervals.clone());
      }

      this.state.line_dash_list = intervals
    }
  }

  Ok(cx.undefined())
}


// line style properties  -----------------------------------------------------------

pub fn get_lineCap(mut cx: FunctionContext) -> JsResult<JsString> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();

  let mode = this.state.paint.stroke_cap();
  let name = from_stroke_cap(mode);
  Ok(cx.string(name))
}

pub fn set_lineCap(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  let name = string_arg(&mut cx, 1, "lineCap")?;

  if let Some(mode) = to_stroke_cap(&name){
    this.state.paint.set_stroke_cap(mode);
  }
  Ok(cx.undefined())
}

pub fn get_lineDashOffset(mut cx: FunctionContext) -> JsResult<JsNumber> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();

  let num = this.state.line_dash_offset;
  Ok(cx.number(num))
}

pub fn set_lineDashOffset(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();

  let num = float_arg(&mut cx, 1, "lineDashOffset")?;
  this.state.line_dash_offset = num;
  Ok(cx.undefined())
}

pub fn get_lineJoin(mut cx: FunctionContext) -> JsResult<JsString> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();

  let mode = this.state.paint.stroke_join();
  let name = from_stroke_join(mode);
  Ok(cx.string(name))
}

pub fn set_lineJoin(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  let name = string_arg(&mut cx, 1, "lineJoin")?;

  if let Some(mode) = to_stroke_join(&name){
    this.state.paint.set_stroke_join(mode);
  }
  Ok(cx.undefined())
}

pub fn get_lineWidth(mut cx: FunctionContext) -> JsResult<JsNumber> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();

  let num = this.state.paint.stroke_width();
  Ok(cx.number(num))
}

pub fn set_lineWidth(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  if let Some(num) = opt_float_arg(&mut cx, 1){
    if num > 0.0 {
      this.state.paint.set_stroke_width(num);
      this.state.stroke_width = num;
    }
  }
  Ok(cx.undefined())
}

pub fn get_miterLimit(mut cx: FunctionContext) -> JsResult<JsNumber> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();

  let num = this.state.paint.stroke_miter();
  Ok(cx.number(num))
}

pub fn set_miterLimit(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  if let Some(num) = opt_float_arg(&mut cx, 1){
    if num > 0.0 {
      this.state.paint.set_stroke_miter(num);
    }
  }
  Ok(cx.undefined())
}


//
// Imagery
//

fn _layout_rects(width:f32, height:f32, nums:&[f32]) -> Option<(Rect, Rect)> {
  let (src, dst) = match nums.len() {
    2 => ( Rect::from_xywh(0.0, 0.0, width, height),
           Rect::from_xywh(nums[0], nums[1], width, height) ),
    4 => ( Rect::from_xywh(0.0, 0.0, width, height),
           Rect::from_xywh(nums[0], nums[1], nums[2], nums[3]) ),
    8 => ( Rect::from_xywh(nums[0], nums[1], nums[2], nums[3]),
           Rect::from_xywh(nums[4], nums[5], nums[6], nums[7]) ),
    _ => return None
  };
  Some((src, dst))
}

pub fn drawRaster(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let source = cx.argument::<BoxedImage>(1)?;
  let image = &source.borrow().image;

  let dims = image.as_ref().map(|img|
    (img.width(), img.height())
  );

  let (width, height) = match dims{
    Some((w,h)) => (w as f32, h as f32),
    None => return cx.throw_error("Cannot draw incomplete image (has it finished loading?)")
  };

  let argc = cx.len() as usize;
  let nums = float_args(&mut cx, 2..argc)?;
  match _layout_rects(width, height, &nums){
    Some((src, dst)) => {
      // shrink src to lie within the image bounds and adjust dst proportionately
      let (src, dst) = fit_bounds(width, height, src, dst);

      let mut this = this.borrow_mut();
      this.draw_image(&image, &src, &dst);
      Ok(cx.undefined())
    },
    None => cx.throw_error(format!("Expected 2, 4, or 8 coordinates (got {})", nums.len()))
  }
}

pub fn drawCanvas(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let canvas = cx.argument::<BoxedCanvas>(1)?;
  let context = cx.argument::<BoxedContext2D>(2)?;

  let canvas = canvas.borrow();
  let (width, height) = (canvas.width as f32, canvas.height as f32);

  let argc = cx.len() as usize;
  let nums = float_args(&mut cx, 3..argc)?;
  match _layout_rects(width, height, &nums){
    Some((src, dst)) => {
      let mut ctx = context.borrow_mut();
      let pict = ctx.get_picture(None);

      let mut this = this.borrow_mut();
      this.draw_picture(&pict, &src, &dst);
      Ok(cx.undefined())
    },
    None => cx.throw_error(format!("Expected 2, 4, or 8 coordinates (got {})", nums.len()))
  }
}

pub fn getImageData(mut cx: FunctionContext) -> JsResult<JsBuffer> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  let x = float_arg(&mut cx, 1, "x")? as i32;
  let y = float_arg(&mut cx, 2, "y")? as i32;
  let width = float_arg(&mut cx, 3, "width")? as i32;
  let height = float_arg(&mut cx, 4, "height")? as i32;

  let buffer = JsBuffer::new(&mut cx, 4 * (width * height) as u32)?;
  cx.borrow(&buffer, |data| {
    this.get_pixels(data.as_mut_slice(), (x, y), (width, height));
  });
  Ok(buffer)
}

pub fn putImageData(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  let img_data = cx.argument::<JsObject>(1)?;

  // determine geometry
  let width = float_for_key(&mut cx, &img_data, "width")?;
  let height = float_for_key(&mut cx, &img_data, "height")?;
  let x = float_arg(&mut cx, 2, "x")?;
  let y = float_arg(&mut cx, 3, "y")?;
  let mut dirty = opt_float_args(&mut cx, 4..8);
  if !dirty.is_empty() && dirty.len() != 4 {
    return cx.throw_type_error("expected either 2 or 6 numbers")
  }
  let (mut src, mut dst) = match dirty.as_mut_slice(){
    [dx, dy, dw, dh] => {
      if *dw < 0.0 { *dw *= -1.0; *dx -= *dw; }
      if *dh < 0.0 { *dh *= -1.0; *dy -= *dh; }
      (Rect::from_xywh(*dx, *dy, *dw, *dh), Rect::from_xywh(*dx + x, *dy + y, *dw, *dh))
    },
    _ => (
      Rect::from_xywh(0.0, 0.0, width, height),
      Rect::from_xywh(x, y, width, height)
  )};

  let buffer = img_data.get(&mut cx, "data")?.downcast_or_throw::<JsBuffer, _>(&mut cx)?;
  let info = Image::info(width, height);
  cx.borrow(&buffer, |data| {
    this.blit_pixels(data.as_slice(), &info, &src, &dst);
  });
  Ok(cx.undefined())
}

// -- image properties --------------------------------------------------------------

pub fn get_imageSmoothingEnabled(mut cx: FunctionContext) -> JsResult<JsBoolean> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  Ok(cx.boolean(this.state.image_smoothing_enabled))
}

pub fn set_imageSmoothingEnabled(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  if let Ok(flag) = bool_arg(&mut cx, 1, "imageSmoothingEnabled"){
    this.state.image_smoothing_enabled = flag;
  }
  Ok(cx.undefined())
}

pub fn get_imageSmoothingQuality(mut cx: FunctionContext) -> JsResult<JsString> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  let mode = from_filter_quality(this.state.image_filter_quality);
  Ok(cx.string(mode))
}

pub fn set_imageSmoothingQuality(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  let name = string_arg(&mut cx, 1, "imageSmoothingQuality")?;

  if let Some(mode) = to_filter_quality(&name){
    this.state.image_filter_quality = mode;
  }
  Ok(cx.undefined())
}

//
// Typography
//


pub fn fillText(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  _draw_text(cx, Fill)
}

pub fn strokeText(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  _draw_text(cx, Stroke)
}

fn _draw_text(mut cx: FunctionContext, ink:PaintStyle) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  let text = string_arg(&mut cx, 1, "text")?;
  let x = float_arg(&mut cx, 2, "x")?;
  let y = float_arg(&mut cx, 3, "y")?;
  let width = opt_float_arg(&mut cx, 4);

  if width.is_none() && cx.len() > 4 && !cx.argument::<JsValue>(4)?.is_a::<JsUndefined, _>(&mut cx){
    // it's fine to include an ignored `undefined` but anything else is invalid
    return Ok(cx.undefined())
  }

  let paint = match ink{
    Stroke => this.paint_for_stroke(),
    _ => this.paint_for_fill(),
  };
  this.draw_text(&text, x, y, width, paint);
  Ok(cx.undefined())
}


pub fn measureText(mut cx: FunctionContext) -> JsResult<JsArray> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  let text = string_arg(&mut cx, 1, "text")?;
  let width = opt_float_arg(&mut cx, 2);
  let text_metrics = this.measure_text(&text, width);

  let results = JsArray::new(&mut cx, text_metrics.len() as u32);
  for (i, info) in text_metrics.iter().enumerate(){
    let line = floats_to_array(&mut cx, &info)?;
    results.set(&mut cx, i as u32, line)?;
  }
  Ok(results)
}

// -- type properties ---------------------------------------------------------------

pub fn get_font(mut cx: FunctionContext) -> JsResult<JsString> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  Ok(cx.string(this.state.font.clone()))
}

pub fn set_font(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  if let Some(spec) = font_arg(&mut cx, 1)?{
    this.set_font(spec);
  }
  Ok(cx.undefined())
}

pub fn get_textAlign(mut cx: FunctionContext) -> JsResult<JsString> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  let mode = from_text_align(this.state.graf_style.text_align());
  Ok(cx.string(mode))
}

pub fn set_textAlign(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  let name = string_arg(&mut cx, 1, "textAlign")?;

  if let Some(mode) = to_text_align(&name){
    this.state.graf_style.set_text_align(mode);
  }
  Ok(cx.undefined())
}

pub fn get_textBaseline(mut cx: FunctionContext) -> JsResult<JsString> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  let mode = from_text_baseline(this.state.text_baseline);
  Ok(cx.string(mode))
}

pub fn set_textBaseline(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  let name = string_arg(&mut cx, 1, "textBaseline")?;

  if let Some(mode) = to_text_baseline(&name){
    this.state.text_baseline = mode;
  }
  Ok(cx.undefined())
}

pub fn get_direction(mut cx: FunctionContext) -> JsResult<JsString> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  let name = match this.state.graf_style.text_direction(){
    TextDirection::LTR => "ltr",
    TextDirection::RTL => "rtl",
  };
  Ok(cx.string(name))
}

pub fn set_direction(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  let name = string_arg(&mut cx, 1, "direction")?;

  let direction = match name.to_lowercase().as_str(){
    "ltr" => Some(TextDirection::LTR),
    "rtl" => Some(TextDirection::RTL),
    _ => None
  };

  if let Some(dir) = direction{
    this.state.graf_style.set_text_direction(dir);
  }
  Ok(cx.undefined())
}

// -- non-standard typography extensions --------------------------------------------

pub fn get_fontVariant(mut cx: FunctionContext) -> JsResult<JsString> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  Ok(cx.string(this.state.font_variant.clone()))
}

pub fn set_fontVariant(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  let arg = cx.argument::<JsObject>(1)?;

  let variant = string_for_key(&mut cx, &arg, "variant")?;
  let feat_obj = arg.get(&mut cx, "features")?.downcast_or_throw::<JsObject, _>(&mut cx)?;
  let features = font_features(&mut cx, &feat_obj)?;
  this.set_font_variant(&variant, &features);
  Ok(cx.undefined())
}

pub fn get_textTracking(mut cx: FunctionContext) -> JsResult<JsNumber> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  Ok(cx.number(this.state.text_tracking))
}

pub fn set_textTracking(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  let tracking = float_arg(&mut cx, 1, "tracking")?;

  let em = this.state.char_style.font_size();
  this.state.text_tracking = tracking as i32;
  this.state.char_style.set_letter_spacing(tracking as f32 / 1000.0 * em);
  Ok(cx.undefined())
}

pub fn get_textWrap(mut cx: FunctionContext) -> JsResult<JsBoolean> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  Ok(cx.boolean(this.state.text_wrap))
}

pub fn set_textWrap(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  let flag = bool_arg(&mut cx, 1, "textWrap")?;
  this.state.text_wrap = flag;
  Ok(cx.undefined())
}

//
// Effects
//

// -- compositing properties --------------------------------------------------------

pub fn get_globalAlpha(mut cx: FunctionContext) -> JsResult<JsNumber> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  Ok(cx.number(this.state.global_alpha))
}

pub fn set_globalAlpha(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  let num = float_arg(&mut cx, 1, "globalAlpha")?;

  if (0.0..=1.0).contains(&num){
    this.state.global_alpha = num;
  }
  Ok(cx.undefined())
}

pub fn get_globalCompositeOperation(mut cx: FunctionContext) -> JsResult<JsString> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  let mode = from_blend_mode(this.state.global_composite_operation);
  Ok(cx.string(mode))
}

pub fn set_globalCompositeOperation(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  let name = string_arg(&mut cx, 1, "globalCompositeOperation")?;

  if let Some(mode) = to_blend_mode(&name){
    this.state.global_composite_operation = mode;
    this.state.paint.set_blend_mode(mode);
  }
  Ok(cx.undefined())
}

// -- css3 filters ------------------------------------------------------------------

pub fn get_filter(mut cx: FunctionContext) -> JsResult<JsString> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  Ok(cx.string(this.state.filter.clone()))
}

pub fn set_filter(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  if !cx.argument::<JsValue>(1)?.is_a::<JsNull, _>(&mut cx) {
    let (filter_text, filters) = filter_arg(&mut cx, 1)?;
    this.set_filter(&filter_text, &filters);
  }
  Ok(cx.undefined())
}

// -- dropshadow properties ---------------------------------------------------------

pub fn get_shadowBlur(mut cx: FunctionContext) -> JsResult<JsNumber> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  Ok(cx.number(this.state.shadow_blur))
}

pub fn set_shadowBlur(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  let num = float_arg(&mut cx, 1, "shadowBlur")?;
  if num >= 0.0{
    this.state.shadow_blur = num;
  }
  Ok(cx.undefined())
}

pub fn get_shadowColor(mut cx: FunctionContext) -> JsResult<JsValue> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  let shadow_color = this.state.shadow_color;
  color_to_css(&mut cx, &shadow_color)
}

pub fn set_shadowColor(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  if let Some(color) = color_arg(&mut cx, 1){
    this.state.shadow_color = color;
  }
  Ok(cx.undefined())
}

pub fn get_shadowOffsetX(mut cx: FunctionContext) -> JsResult<JsNumber> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  Ok(cx.number(this.state.shadow_offset.x))
}

pub fn get_shadowOffsetY(mut cx: FunctionContext) -> JsResult<JsNumber> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  Ok(cx.number(this.state.shadow_offset.y))
}

pub fn set_shadowOffsetX(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  let num = float_arg(&mut cx, 1, "shadowOffsetX")?;
  this.state.shadow_offset.x = num;
  Ok(cx.undefined())
}

pub fn set_shadowOffsetY(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  let num = float_arg(&mut cx, 1, "shadowOffsetY")?;
  this.state.shadow_offset.y = num;
  Ok(cx.undefined())
}