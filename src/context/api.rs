#![allow(unused_variables)]
#![allow(unused_mut)]
#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(non_snake_case)]
use std::f32::consts::PI;
use std::cell::RefCell;
use neon::prelude::*;
use skia_safe::{Point, Rect, Matrix, Path, PathDirection, PaintStyle};
use skia_safe::path::AddPathMode::Append;
use skia_safe::path::AddPathMode::Extend;
use skia_safe::textlayout::{TextDirection};
use skia_safe::PaintStyle::{Fill, Stroke};

use super::{Context2D, BoxedContext2D, Dye};
use crate::canvas::{Canvas, BoxedCanvas};
use crate::path::{Path2D, BoxedPath2D};
use crate::image::{Image, BoxedImage};
use crate::typography::*;
use crate::utils::*;

//
// The js interface for the Context2D struct
//

pub fn new(mut cx: FunctionContext) -> JsResult<BoxedContext2D> {
  let this = RefCell::new(Context2D::new());
  let parent = cx.argument::<BoxedCanvas>(1)?;
  let parent = parent.borrow();

  this.borrow_mut().resize((parent.width, parent.height));
  Ok(cx.boxed(this))
}

pub fn resetSize(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let parent = cx.argument::<BoxedCanvas>(1)?;
  let parent = parent.borrow();

  this.borrow_mut().resize((parent.width, parent.height));
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
  check_argc(&mut cx, 7)?;

  let nums = opt_float_args(&mut cx, 1..7);
  if let [m11, m12, m21, m22, dx, dy] = nums.as_slice(){
    let matrix = Matrix::new_all(*m11, *m21, *dx, *m12, *m22, *dy, 0.0, 0.0, 1.0);
    this.with_matrix(|ctm| ctm.pre_concat(&matrix) );
  }
  Ok(cx.undefined())
}

pub fn translate(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  check_argc(&mut cx, 3)?;

  let xy = opt_float_args(&mut cx, 1..3);
  if let [dx, dy] = xy.as_slice(){
    this.with_matrix(|ctm| ctm.pre_translate((*dx, *dy)) );
  }
  Ok(cx.undefined())
}

pub fn scale(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  check_argc(&mut cx, 3)?;

  let xy = opt_float_args(&mut cx, 1..3);
  if let [m11, m22] = xy.as_slice(){
    this.with_matrix(|ctm| ctm.pre_scale((*m11, *m22), None) );
  }
  Ok(cx.undefined())
}

pub fn rotate(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  check_argc(&mut cx, 2)?;

  if let Some(radians) = opt_float_arg(&mut cx, 1){
    let degrees = radians / PI * 180.0;
    this.with_matrix(|ctm| ctm.pre_rotate(degrees, None) );
  }
  Ok(cx.undefined())
}

pub fn resetTransform(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();

  this.with_matrix(|ctm| ctm.reset() );
  Ok(cx.undefined())
}

pub fn createProjection(mut cx: FunctionContext) -> JsResult<JsArray> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  let dst = points_arg(&mut cx, 1)?;
  let src = points_arg(&mut cx, 2)?;

  let basis:Vec<Point> = match src.len(){
    0 => this.bounds.to_quad().to_vec(), // use canvas dims
    1 => Rect::from_wh(src[0].x, src[0].y).to_quad().to_vec(), // implicit 0,0 origin
    2 => Rect::new(src[0].x, src[0].y, src[1].x, src[1].y).to_quad().to_vec(), // lf/top, rt/bot
    _ => src.clone(),
  };

  let quad:Vec<Point> = match dst.len(){
    1 => Rect::from_wh(dst[0].x, dst[0].y).to_quad().to_vec(), // implicit 0,0 origin
    2 => Rect::new(dst[0].x, dst[0].y, dst[1].x, dst[1].y).to_quad().to_vec(), // lf/top, rt/bot
    _ => dst.clone(),
  };

  match (Matrix::from_poly_to_poly(&basis, &quad), basis.len() == quad.len()){
    (Some(projection), true) => {
      let array = JsArray::new(&mut cx, 9);
      for i in 0..9 {
        let num = cx.number(projection[i as usize]);
        array.set(&mut cx, i as u32, num)?;
      }
      Ok(array)
    },
    _ => cx.throw_type_error(format!(
      "Expected 2 or 4 x/y points for output quad (got {}) and 0, 1, 2, or 4 points for the coordinate basis (got {})",
      quad.len(), basis.len()
    ))
  }
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

  if let Some(matrix) = opt_matrix_arg(&mut cx, 1){
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
  check_argc(&mut cx, 5)?;

  let nums = opt_float_args(&mut cx, 1..5);
  if let [x, y, w, h] = nums.as_slice(){
    let rect = Rect::from_xywh(*x, *y, *w, *h);
    let quad = this.state.matrix.map_rect_to_quad(rect);
    this.path.move_to(quad[0]);
    this.path.line_to(quad[1]);
    this.path.line_to(quad[2]);
    this.path.line_to(quad[3]);
    this.path.close();
  }
  Ok(cx.undefined())
}

pub fn arc(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  check_argc(&mut cx, 6)?;

  let nums = opt_float_args(&mut cx, 1..6);
  let ccw = bool_arg_or(&mut cx, 6, false);
  if let [x, y, radius, start_angle, end_angle] = nums.as_slice(){
    let matrix = this.state.matrix;
    let mut arc = Path2D::new();
    arc.add_ellipse((*x, *y), (*radius, *radius), 0.0, *start_angle, *end_angle, ccw);
    this.path.add_path(&arc.path.with_transform(&matrix), (0,0), Extend);
  }
  Ok(cx.undefined())
}

pub fn ellipse(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  check_argc(&mut cx, 8)?;

  let nums = opt_float_args(&mut cx, 1..8);
  let ccw = bool_arg_or(&mut cx, 8, false);
  if let [x, y, x_radius, y_radius, rotation, start_angle, end_angle] = nums.as_slice(){
    if *x_radius < 0.0 || *y_radius < 0.0 {
      return cx.throw_error("radii cannot be negative")
    }
    let matrix = this.state.matrix;
    let mut arc = Path2D::new();
    arc.add_ellipse((*x, *y), (*x_radius, *y_radius), *rotation, *start_angle, *end_angle, ccw);
    this.path.add_path(&arc.path.with_transform(&matrix), (0,0), Extend);
  }
  Ok(cx.undefined())
}

// contour drawing ----------------------------------------------------------------------

pub fn moveTo(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  check_argc(&mut cx, 3)?;

  let xy = opt_float_args(&mut cx, 1..3);
  if let Some(dst) = this.map_points(&xy).first(){
    this.path.move_to(*dst);
  }
  Ok(cx.undefined())
}

pub fn lineTo(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  check_argc(&mut cx, 3)?;

  let xy = opt_float_args(&mut cx, 1..3);
  if let Some(dst) = this.map_points(&xy).first(){
    if this.path.is_empty(){ this.path.move_to(*dst); }
    this.path.line_to(*dst);
  }
  Ok(cx.undefined())
}

pub fn arcTo(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  check_argc(&mut cx, 6)?;

  let coords = opt_float_args(&mut cx, 1..5);
  let radius = opt_float_arg(&mut cx, 5);
  if let Some(radius) = radius {
    if let [src, dst] = this.map_points(&coords).as_slice(){
      if this.path.is_empty(){ this.path.move_to(*src); }
      this.path.arc_to_tangent(*src, *dst, radius);
    }
  }
  Ok(cx.undefined())
}

pub fn bezierCurveTo(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  check_argc(&mut cx, 7)?;

  let coords = opt_float_args(&mut cx, 1..7);
  if let [cp1, cp2, dst] = this.map_points(&coords).as_slice(){
    if this.path.is_empty(){ this.path.move_to(*cp1); }
    this.path.cubic_to(*cp1, *cp2, *dst);
  }
  Ok(cx.undefined())
}

pub fn quadraticCurveTo(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  check_argc(&mut cx, 5)?;

  let coords = opt_float_args(&mut cx, 1..5);
  if let [cp, dst] = this.map_points(&coords).as_slice(){
    if this.path.is_empty(){ this.path.move_to(*cp); }
    this.path.quad_to(*cp, *dst);
  }
  Ok(cx.undefined())
}


pub fn conicCurveTo(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  check_argc(&mut cx, 6)?;

  let coords = opt_float_args(&mut cx, 1..5);
  let weight = opt_float_arg(&mut cx, 5);
  if let Some(weight) = weight {
    if let [src, dst] = this.map_points(&coords).as_slice(){
      if this.path.is_empty(){ this.path.move_to((src.x, src.y)); }
      this.path.conic_to((src.x, src.y), (dst.x, dst.y), weight);
    }
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
  let clip = opt_path2d_arg(&mut cx, 1);
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
  let path = opt_path2d_arg(&mut cx, 1);
  let rule_idx = if path.is_some(){ 2 }else{ 1 };
  let rule = fill_rule_arg_or(&mut cx, rule_idx, "nonzero")?;
  this.borrow_mut().draw_path(path, PaintStyle::Fill, Some(rule));
  Ok(cx.undefined())
}

pub fn stroke(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let path = opt_path2d_arg(&mut cx, 1);
  this.borrow_mut().draw_path(path, PaintStyle::Stroke, None);
  Ok(cx.undefined())
}

pub fn fillRect(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let nums = float_args(&mut cx, 1..5)?;
  if let [x, y, w, h] = nums.as_slice() {
    let rect = Rect::from_xywh(*x, *y, *w, *h);
    let path = Path::rect(rect, None);
    this.borrow_mut().draw_path(Some(path), PaintStyle::Fill, None);
  }
  Ok(cx.undefined())
}

pub fn strokeRect(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let nums = float_args(&mut cx, 1..5)?;
  if let [x, y, w, h] = nums.as_slice() {
    let rect = Rect::from_xywh(*x, *y, *w, *h);
    let path = Path::rect(rect, None);
    this.borrow_mut().draw_path(Some(path), PaintStyle::Stroke, None);
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
  dye.value(&mut cx)
}

pub fn set_fillStyle(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  let arg = cx.argument::<JsValue>(1)?;

  if let Some(dye) = Dye::new(&mut cx, arg) {
    this.state.fill_style = dye;
  }
  Ok(cx.undefined())
}

pub fn get_strokeStyle(mut cx: FunctionContext) -> JsResult<JsValue> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let this = this.borrow();
  let dye = this.state.stroke_style.clone();
  dye.value(&mut cx)
}

pub fn set_strokeStyle(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  let arg = cx.argument::<JsValue>(1)?;

  if let Some(dye) = Dye::new(&mut cx, arg) {
    this.state.stroke_style = dye;
  }
  Ok(cx.undefined())
}

//
// Line Style
//

pub fn set_lineDashMarker(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let marker = opt_path2d_arg(&mut cx, 1);

  if marker.is_none(){
    let val = cx.argument::<JsValue>(1)?;
    if !(val.is_a::<JsNull, _>(&mut cx) || val.is_a::<JsNull, _>(&mut cx)){
      return cx.throw_type_error("Expected a Path2D object (or null)");
    }
  }

  this.borrow_mut().state.line_dash_marker = marker;
  Ok(cx.undefined())
}

pub fn get_lineDashMarker(mut cx: FunctionContext) -> JsResult<JsValue> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let this = this.borrow();

  match &this.state.line_dash_marker{
    Some(marker) => Ok(cx.boxed(RefCell::new(Path2D{path:marker.clone()})).upcast()),
    None => Ok(cx.null().upcast())
  }
}

pub fn set_lineDashFit(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let style = string_arg(&mut cx, 1, "fitStyle")?;

  if let Some(fit) = to_1d_style(&style){
    this.borrow_mut().state.line_dash_fit = fit;
  }
  Ok(cx.undefined())
}

pub fn get_lineDashFit(mut cx: FunctionContext) -> JsResult<JsString> {
  let this = cx.argument::<BoxedContext2D>(0)?;

  let fit = from_1d_style(this.borrow().state.line_dash_fit);
  Ok(cx.string(fit))
}

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
      .filter(|n| *n >= 0.0 && n.is_finite())
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

  if let Some(num) = opt_float_arg(&mut cx, 1){
    this.state.line_dash_offset = num;
  }
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

pub fn drawImage(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let source = cx.argument::<JsValue>(1)?;
  let image = {
    if let Ok(obj) = source.downcast::<BoxedImage, _>(&mut cx){
      (&obj.borrow().image).clone()
    }else if let Ok(obj) = source.downcast::<BoxedContext2D, _>(&mut cx){
      obj.borrow().get_image()
    }else{
      return Ok(cx.undefined())
    }
  };

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
  let context = cx.argument::<BoxedContext2D>(1)?;

  let (width, height) = {
    let bounds = context.borrow().bounds;
    (bounds.width(), bounds.height())
  };

  let argc = cx.len() as usize;
  let nums = float_args(&mut cx, 2..argc)?;
  match _layout_rects(width, height, &nums){
    Some((src, dst)) => {
      let pict = {
        let mut ctx = context.borrow_mut();
        ctx.get_picture()
      };

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
  let antialias = this.antialias;

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
    this.blit_pixels(data.as_slice(), &info, &src, &dst ,antialias);
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
  let flag = bool_arg(&mut cx, 1, "imageSmoothingEnabled")?;

  this.state.image_smoothing_enabled = flag;
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

fn _draw_text(mut cx: FunctionContext, style:PaintStyle) -> JsResult<JsUndefined> {
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

  this.draw_text(&text, x, y, width, style);
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
    let line = floats_to_array(&mut cx, info)?;
    results.set(&mut cx, i as u32, line)?;
  }
  Ok(results)
}

pub fn outlineText(mut cx: FunctionContext) -> JsResult<JsValue> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let text = string_arg(&mut cx, 1, "text")?;
  let mut this = this.borrow_mut();
  if let Some(path) = this.outline_text(&text){
    Ok(cx.boxed(RefCell::new(Path2D{path})).upcast())
  }else{
    Ok(cx.undefined().upcast())
  }
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
  if let Some(num) = opt_float_arg(&mut cx, 1){
    if num >= 0.0 {
      this.state.shadow_blur = num;
    }
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
  if let Some(num) = opt_float_arg(&mut cx, 1){
    this.state.shadow_offset.x = num;
  }
  Ok(cx.undefined())
}

pub fn set_shadowOffsetY(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  if let Some(num) = opt_float_arg(&mut cx, 1){
    this.state.shadow_offset.y = num;
  }
  Ok(cx.undefined())
}

pub fn set_antialias(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  let flag = string_arg(&mut cx, 1, "antialias")?;
  let antialias = match flag.to_lowercase().as_str(){"none" => false,_ => true};
  this.set_antialias(antialias);
  Ok(cx.undefined())
}

pub fn get_antialias(mut cx: FunctionContext) -> JsResult<JsString> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  let antialias:bool = this.antialias;
  if antialias {
    Ok(cx.string("default"))
  }else{
    Ok(cx.string("none"))
  }
  
}