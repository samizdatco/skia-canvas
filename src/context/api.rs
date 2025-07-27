#![allow(non_snake_case)]
use std::f32::consts::PI;
use std::cell::RefCell;
use neon::prelude::*;
use skia_safe::{Matrix, PaintStyle, Point, RRect, Rect, Size};
use skia_safe::path::{AddPathMode::{Extend}, Direction::{CCW, CW}, Path};
use skia_safe::textlayout::{TextDirection};
use skia_safe::PaintStyle::{Fill, Stroke};

use super::{Context2D, BoxedContext2D, Dye, page::ExportOptions};
use crate::canvas::BoxedCanvas;
use crate::path::Path2D;
use crate::image::{BoxedImage, Content};
use crate::filter::Filter;
use crate::typography::{
  font_arg, decoration_arg, font_features, from_width, to_width,
  from_text_align, to_text_align, from_text_baseline, to_text_baseline,
  opt_spacing_arg
};
use crate::utils::*;

//
// The js interface for the Context2D struct
//

pub fn new(mut cx: FunctionContext) -> JsResult<BoxedContext2D> {
  let this = RefCell::new(Context2D::new());
  let parent = cx.argument::<BoxedCanvas>(1)?;
  let parent = parent.borrow();

  this.borrow_mut().reset_size((parent.width, parent.height));
  Ok(cx.boxed(this))
}

pub fn resetSize(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let parent = cx.argument::<BoxedCanvas>(1)?;
  let parent = parent.borrow();

  this.borrow_mut().reset_size((parent.width, parent.height));
  Ok(cx.undefined())
}

pub fn get_size(mut cx: FunctionContext) -> JsResult<JsArray> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let bounds = this.borrow().bounds;

  let array = JsArray::new(&mut cx, 2);
  let width = cx.number(bounds.size().width);
  let height = cx.number(bounds.size().height);
  array.set(&mut cx, 0, width)?;
  array.set(&mut cx, 1, height)?;
  Ok(array)
}

pub fn set_size(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;

  if let [width, height] = opt_float_args(&mut cx, 1..3).as_slice(){
    this.borrow_mut().resize((*width, *height));
  }
  Ok(cx.undefined())
}

pub fn reset(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  let size = this.bounds.size();

  this.reset_size(size);
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

  if let Some(matrix) = opt_matrix_arg(&mut cx, 1) {
    this.with_matrix(|ctm| ctm.pre_concat(&matrix) );
  }
  Ok(cx.undefined())
}

pub fn translate(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();

  let xy = float_args_or_bail(&mut cx, &["x", "y"])?;
  if let [dx, dy] = xy.as_slice(){
    this.with_matrix(|ctm| ctm.pre_translate((*dx, *dy)) );
  }
  Ok(cx.undefined())
}

pub fn scale(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();

  let xy = float_args_or_bail(&mut cx, &["x", "y"])?;
  if let [m11, m22] = xy.as_slice(){
    this.with_matrix(|ctm| ctm.pre_scale((*m11, *m22), None) );
  }
  Ok(cx.undefined())
}

pub fn rotate(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();

  let radians = float_arg_or_bail(&mut cx, 1, "angle")?;
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

pub fn createProjection(mut cx: FunctionContext) -> JsResult<JsArray> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let this = this.borrow_mut();
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

  let nums = float_args_or_bail(&mut cx, &["x", "y", "width", "height"])?;
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

pub fn roundRect(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();

  let nums = float_args(&mut cx, &[
    "x", "y", "width", "height", "r1x", "r1y", "r2x", "r2y", "r3x", "r3y", "r4x", "r4y"
  ])?;
  if let [x, y, w, h] = &nums[..4]{
    let rect = Rect::from_xywh(*x, *y, *w, *h);
    let radii:Vec<Point> = nums[4..].chunks(2).map(|xy| Point::new(xy[0], xy[1])).collect();
    let rrect = RRect::new_rect_radii(rect, &[radii[0], radii[1], radii[2], radii[3]]);
    let direction = if w.signum() == h.signum(){ CW }else{ CCW };

    let matrix = this.state.matrix;
    let path = Path::rrect(rrect, Some(direction));
    this.path.add_path(&path.with_transform(&matrix), (0,0), Extend);
  }

  Ok(cx.undefined())
}

pub fn arc(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();

  let nums = float_args_or_bail(&mut cx, &["x", "y", "radius", "startAngle", "endAngle"])?;
  let ccw = bool_arg_or(&mut cx, 6, false);
  if let [x, y, radius, start_angle, end_angle] = nums.as_slice(){
    let matrix = this.state.matrix;
    let mut arc = Path2D::default();
    arc.add_ellipse((*x, *y), (*radius, *radius), 0.0, *start_angle, *end_angle, ccw);
    this.path.add_path(&arc.path.with_transform(&matrix), (0,0), Extend);
  }
  Ok(cx.undefined())
}

pub fn ellipse(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();

  let nums = float_args_or_bail(&mut cx, &["x", "y", "xRadius", "yRadius", "rotation", "startAngle", "endAngle"])?;
  let ccw = bool_arg_or(&mut cx, 8, false);
  if let [x, y, x_radius, y_radius, rotation, start_angle, end_angle] = nums.as_slice(){
    if *x_radius < 0.0 || *y_radius < 0.0 {
      return cx.throw_range_error("Radius value must be positive")
    }
    let matrix = this.state.matrix;
    let mut arc = Path2D::default();
    arc.add_ellipse((*x, *y), (*x_radius, *y_radius), *rotation, *start_angle, *end_angle, ccw);
    this.path.add_path(&arc.path.with_transform(&matrix), (0,0), Extend);
  }
  Ok(cx.undefined())
}

// contour drawing ----------------------------------------------------------------------

pub fn moveTo(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();

  let xy = float_args_or_bail(&mut cx, &["x", "y"])?;
  if let Some(dst) = this.map_points(&xy).first(){
    this.path.move_to(*dst);
  }
  Ok(cx.undefined())
}

pub fn lineTo(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();

  let xy = float_args_or_bail(&mut cx, &["x", "y"])?;
  if let Some(dst) = this.map_points(&xy).first(){
    this.scoot(*dst);
    this.path.line_to(*dst);
  }
  Ok(cx.undefined())
}

pub fn arcTo(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();

  let coords = float_args_or_bail(&mut cx, &["x1", "y1", "x2", "y2"])?;
  let radius = float_arg_or_bail(&mut cx, 5, "radius")?;
  if radius < 0.0 {
    return cx.throw_range_error("Radius value must be positive")
  }

  if let [src, dst] = this.map_points(&coords)[..2]{
    this.scoot(src);
    this.path.arc_to_tangent(src, dst, radius);
  }
  Ok(cx.undefined())
}

pub fn bezierCurveTo(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();

  let coords = float_args_or_bail(&mut cx, &["cp1x", "cp1y", "cp2x", "cp2y", "x", "y"])?;
  if let [cp1, cp2, dst] = this.map_points(&coords)[..3]{
    this.scoot(cp1);
    this.path.cubic_to(cp1, cp2, dst);
  }
  Ok(cx.undefined())
}

pub fn quadraticCurveTo(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();

  let coords = float_args_or_bail(&mut cx, &["cpx", "cpy", "x", "y"])?;
  if let [cp, dst] = this.map_points(&coords)[..2]{
    this.scoot(cp);
    this.path.quad_to(cp, dst);
  }
  Ok(cx.undefined())
}


pub fn conicCurveTo(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();

  let args = float_args_or_bail(&mut cx, &["cpx", "cpy", "x", "y", "weight"])?;
  if let [src, dst] = this.map_points(&args[..4]).as_slice(){
    this.scoot(*src);
    this.path.conic_to((src.x, src.y), (dst.x, dst.y), args[4]);
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

pub fn isPointInPath(cx: FunctionContext) -> JsResult<JsBoolean> {
  _is_in(cx, Fill)
}

pub fn isPointInStroke(cx: FunctionContext) -> JsResult<JsBoolean> {
  _is_in(cx, Stroke)
}

fn _is_in(mut cx: FunctionContext, style:PaintStyle) -> JsResult<JsBoolean> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();

  let path = opt_skpath_arg(&mut cx, 1);
  let (rule_idx, mut target) = match path{
    Some(path) => (4, path),
    None => match cx.len(){
      5 => cx.throw_type_error("Expected a Path2D for 1st arg")?,
      _ => (3, this.path.clone())
    }
  };

  let rule = match style{
    Stroke => None,
    _ => Some(fill_rule_arg_or(&mut cx, rule_idx, "nonzero")?)
  };

  if let [x, y] = opt_float_args(&mut cx, 1..4).as_slice(){
    Ok(cx.boolean(this.hit_test_path(&mut target, (*x, *y), rule, style)))
  }else{
    check_argc(&mut cx, 3)?;
    Ok(cx.boolean(false))
  }

}

// masking ------------------------------------------------------------------------------

pub fn clip(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();

  let mut shift = 1;
  let path = opt_skpath_arg(&mut cx, 1);
  if path.is_some() { shift += 1; }
  else if cx.len() > 2{
    return cx.throw_type_error("Expected a Path2D for 1st arg")
  }
  let rule = fill_rule_arg_or(&mut cx, shift, "nonzero")?;

  this.clip_path(path, rule);
  Ok(cx.undefined())
}


//
// Fill & Stroke
//

pub fn fill(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();

  let mut shift = 1;
  let path = opt_skpath_arg(&mut cx, 1);
  if path.is_some() { shift += 1; }
  else if cx.len() > 2{
    return cx.throw_type_error("Expected a Path2D for 1st arg")
  }
  let rule = fill_rule_arg_or(&mut cx, shift, "nonzero")?;

  this.draw_path(path, PaintStyle::Fill, Some(rule));
  Ok(cx.undefined())
}

pub fn stroke(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let path = opt_skpath_arg(&mut cx, 1);

  if path.is_none() && cx.len() >= 2{
    return cx.throw_type_error(format!("Expected a Path2D for 1st arg"))
  }

  this.borrow_mut().draw_path(path, PaintStyle::Stroke, None);
  Ok(cx.undefined())
}

pub fn fillRect(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;

  let nums = float_args_or_bail(&mut cx, &["x", "y", "width", "height"])?;
  if let [x, y, w, h] = nums.as_slice() {
    let rect = Rect::from_xywh(*x, *y, *w, *h);
    let path = Path::rect(rect, None);
    this.borrow_mut().draw_path(Some(path), PaintStyle::Fill, None);
  }
  Ok(cx.undefined())
}

pub fn strokeRect(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;

  let nums = float_args_or_bail(&mut cx, &["x", "y", "width", "height"])?;
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

  let nums = float_args_or_bail(&mut cx, &["x", "y", "width", "height"])?;
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
  let marker = opt_skpath_arg(&mut cx, 1);

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
  let this = this.borrow_mut();
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

    // only apply if all elements were actually numbers
    if list.len() == intervals.len(){
      if intervals.len() % 2 == 1{
        intervals.append(&mut intervals.clone());
      }

      this.state.line_dash_list = intervals
    }
  }else{
    cx.throw_type_error("Value is not a sequence")?
  }

  Ok(cx.undefined())
}


// line style properties  -----------------------------------------------------------

pub fn get_lineCap(mut cx: FunctionContext) -> JsResult<JsString> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let this = this.borrow_mut();

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
  let this = this.borrow_mut();

  let num = this.state.line_dash_offset;
  Ok(cx.number(num))
}

pub fn set_lineDashOffset(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();

  this.state.line_dash_offset = float_arg_or_bail(&mut cx, 1, "lineDashOffset")?;
  Ok(cx.undefined())
}

pub fn get_lineJoin(mut cx: FunctionContext) -> JsResult<JsString> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let this = this.borrow_mut();

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
  let this = this.borrow_mut();

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
  let this = this.borrow_mut();

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

fn _layout_rects(cx: &mut FunctionContext, intrinsic:Size, nums:&[f32]) -> NeonResult<(Rect, Rect)> {
  let (src, dst) = match nums.len() {
    2 => ( Rect::from_xywh(0.0, 0.0, intrinsic.width, intrinsic.height),
           Rect::from_xywh(nums[0], nums[1], intrinsic.width, intrinsic.height) ),
    4 => ( Rect::from_xywh(0.0, 0.0, intrinsic.width, intrinsic.height),
           Rect::from_xywh(nums[0], nums[1], nums[2], nums[3]) ),
    8 => ( Rect::from_xywh(nums[0], nums[1], nums[2], nums[3]),
           Rect::from_xywh(nums[4], nums[5], nums[6], nums[7]) ),
    9.. => cx.throw_type_error(format!("⚠️Expected 2, 4, or 8 coordinates (got {})", nums.len()))?,
    _ => cx.throw_type_error(format!("not enough arguments: Expected 2, 4, or 8 coordinates (got {})", nums.len()))?
  };

  match intrinsic.is_empty(){
    true => cx.throw_range_error(format!("Dimensions must be non-zero (got {}×{})", intrinsic.width, intrinsic.height)),
    false => Ok((src, dst))
  }
}

pub fn drawImage(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let argc = cx.len() as usize;
  let source = cx.argument::<JsValue>(1)?;
  let arg_names = ["srcX", "srcY", "srcWidth", "srcHeight", "dstX", "dstY", "dstWidth", "dstHeight"];
  let nums = float_args_or_bail_at(&mut cx, 2, &arg_names[..argc-2])?;

  let content = {
    if let Ok(img) = source.downcast::<BoxedImage, _>(&mut cx){
      img.borrow().content.clone()
    }else if let Ok(ctx) = source.downcast::<BoxedContext2D, _>(&mut cx){
      Content::from_context(&mut ctx.borrow_mut(), false)
    }else if let Ok(image_data) = image_data_arg(&mut cx, 1){
      Content::from_image_data(image_data)
    }else{
      Content::default()
    }
  };

  if let Content::Bitmap(img) = &content {
    let bounds_size = content.size();
    let (src, dst) = _layout_rects(&mut cx, bounds_size, &nums)?;

    content.snap_rects_to_bounds(src, dst);
    let mut this = this.borrow_mut();
    this.draw_image(&img, &src, &dst);
  } else if let Content::Vector(pict) = &content {
    let image = source.downcast::<BoxedImage, _>(&mut cx).unwrap();
    let fit_to_canvas = image.borrow().autosized;
    let pict_size = content.size();
    let (mut src, mut dst) = _layout_rects(&mut cx, pict_size, &nums)?;

    // for SVG images with no intrinsic size, use the canvas size as a default scale
    if fit_to_canvas && nums.len() != 4 {
      let canvas_size = this.borrow().bounds.size();
      let canvas_min = canvas_size.width.min(canvas_size.height);
      let pict_min = pict_size.width.min(pict_size.height);

      if nums.len() == 2 {
        // if the user doesn't specify a size, proportionally scale to fit within canvas
        let factor = canvas_min / pict_min;
        dst = Rect::from_point_and_size((dst.x(), dst.y()), dst.size() * factor);
      } else if nums.len() == 8 {
        // if clipping out part of the source, map the crop coordinates as if the image is canvas-sized
        let factor = (pict_size.width / canvas_min, pict_size.height / canvas_min);
        (src, _) = Matrix::scale(factor).map_rect(src);
      }
    }

    content.snap_rects_to_bounds(src, dst);
    let mut this = this.borrow_mut();
    this.draw_picture(&pict, &src, &dst);
  }

  Ok(cx.undefined())
}

pub fn drawCanvas(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let argc = cx.len() as usize;
  let this = cx.argument::<BoxedContext2D>(0)?;
  let context = cx.argument::<BoxedContext2D>(1)?;
  let arg_names = ["srcX", "srcY", "srcWidth", "srcHeight", "dstX", "dstY", "dstWidth", "dstHeight"];
  let nums = float_args_or_bail_at(&mut cx, 2, &arg_names[..argc-2])?;

  let content = Content::from_context(&mut context.borrow_mut(), true);
  if let Content::Vector(pict) = &content{
    let (src, dst) = _layout_rects(&mut cx, content.size(), &nums)?;
    let (src, dst) = content.snap_rects_to_bounds(src, dst);
    this.borrow_mut().draw_picture(&pict, &src, &dst);
    Ok(cx.undefined())
  }else{
    cx.throw_error("Canvas's PictureRecorder failed to generate an image")
  }
}

pub fn getImageData(mut cx: FunctionContext) -> JsResult<JsBuffer> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut x = float_arg(&mut cx, 1, "x")?.floor();
  let mut y = float_arg(&mut cx, 2, "y")?.floor();
  let mut w = float_arg(&mut cx, 3, "width")?.floor();
  let mut h = float_arg(&mut cx, 4, "height")?.floor();
  let (color_type, color_space, matte, density, msaa) = image_data_export_arg(&mut cx, 5);
  let parent = cx.argument::<BoxedCanvas>(6)?;
  let canvas = &mut parent.borrow_mut();

  // negative dimensions are valid, just shift the origin and absify
  if w < 0.0 { x += w; w *= -1.0; }
  if h < 0.0 { y += h; h *= -1.0; }

  let opts = ExportOptions{matte, density, msaa, color_type, color_space, ..canvas.export_options()};
  let crop = Rect::from_point_and_size((x*density, y*density), (w*density, h*density)).round();
  let engine = canvas.engine();

  let data = this.borrow_mut().get_pixels(crop, opts, engine).or_else(|e| cx.throw_error(e))?;
  let buffer = JsBuffer::from_slice(&mut cx, &data)?;

  Ok(buffer)
}

pub fn putImageData(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  let img_data = image_data_arg(&mut cx, 1)?;

  // determine geometry
  let x = float_arg(&mut cx, 2, "dx")?;
  let y = float_arg(&mut cx, 3, "dy")?;
  let mut dirty = match cx.len(){
    5.. => float_args_at(&mut cx, 4, &["dirtyX", "dirtyY", "dirtyWidth", "dirtyHeight"])?,
    _ => [].to_vec()
  };
  let (src, dst) = match dirty.as_mut_slice(){
    [dx, dy, dw, dh] => {
      // negative dimensions are valid, just shift the origin and absify
      if *dw < 0.0 { *dw *= -1.0; *dx -= *dw; }
      if *dh < 0.0 { *dh *= -1.0; *dy -= *dh; }
      (Rect::from_xywh(*dx, *dy, *dw, *dh), Rect::from_xywh(*dx + x, *dy + y, *dw, *dh))
    },
    _ => (
      Rect::from_xywh(0.0, 0.0, img_data.width, img_data.height),
      Rect::from_xywh(x, y, img_data.width, img_data.height)
  )};

  this.blit_pixels(img_data, &src, &dst);
  Ok(cx.undefined())
}

// -- image properties --------------------------------------------------------------

pub fn get_imageSmoothingEnabled(mut cx: FunctionContext) -> JsResult<JsBoolean> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let this = this.borrow_mut();
  // Ok(cx.boolean(this.state.image_smoothing_enabled))
  Ok(cx.boolean(this.state.image_filter.smoothing))
}

pub fn set_imageSmoothingEnabled(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  let flag = bool_arg(&mut cx, 1, "imageSmoothingEnabled")?;

  this.state.image_filter.smoothing = flag;
  Ok(cx.undefined())
}

pub fn get_imageSmoothingQuality(mut cx: FunctionContext) -> JsResult<JsString> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let this = this.borrow_mut();
  let mode = from_filter_quality(this.state.image_filter.quality);
  Ok(cx.string(mode))
}

pub fn set_imageSmoothingQuality(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  let name = string_arg(&mut cx, 1, "imageSmoothingQuality")?;

  if let Some(mode) = to_filter_quality(&name){
    this.state.image_filter.quality = mode;
  }
  Ok(cx.undefined())
}

//
// Typography
//


pub fn fillText(cx: FunctionContext) -> JsResult<JsUndefined> {
  _draw_text(cx, Fill)
}

pub fn strokeText(cx: FunctionContext) -> JsResult<JsUndefined> {
  _draw_text(cx, Stroke)
}

fn _draw_text(mut cx: FunctionContext, style:PaintStyle) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  let text = string_arg(&mut cx, 1, "text")?;
  let x = float_arg_or_bail(&mut cx, 2, "x")?;
  let y = float_arg_or_bail(&mut cx, 3, "y")?;
  let width = opt_float_arg(&mut cx, 4);

  // it's fine to include an ignored `undefined` but anything else is invalid
  if width.is_none() && cx.len() > 4 && !cx.argument::<JsValue>(4)?.is_a::<JsUndefined, _>(&mut cx){
    // emoji indicates that it will only throw in strict mode
    cx.throw_type_error("⚠️Expected a number for `width` as 4th arg")?
  }

  this.draw_text(&text, x, y, width, style);
  Ok(cx.undefined())
}


pub fn measureText(mut cx: FunctionContext) -> JsResult<JsString> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  let text = string_arg(&mut cx, 1, "text")?;
  let width = opt_float_arg(&mut cx, 2);
  let text_metrics = this.measure_text(&text, width);
  Ok(cx.string(text_metrics.to_string()))
}

pub fn outlineText(mut cx: FunctionContext) -> JsResult<JsValue> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let this = this.borrow_mut();

  let text = string_arg(&mut cx, 1, "text")?;
  let width = match cx.len(){
    3 => Some(float_arg_or_bail(&mut cx, 2, "width")?),
    _ => None
  };
  let path = this.outline_text(&text, width);
  Ok(cx.boxed(RefCell::new(Path2D{path})).upcast())
}

// -- type properties ---------------------------------------------------------------

pub fn get_font(mut cx: FunctionContext) -> JsResult<JsString> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let this = this.borrow_mut();
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

pub fn get_fontStretch(mut cx: FunctionContext) -> JsResult<JsString> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let this = this.borrow_mut();
  Ok(cx.string(from_width(this.state.font_width)))
}

pub fn set_fontStretch(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  if let Some(stretch) = opt_string_arg(&mut cx, 1){
    let mut this = this.borrow_mut();
    this.set_font_width(to_width(&stretch));
  }
  Ok(cx.undefined())
}

pub fn get_textAlign(mut cx: FunctionContext) -> JsResult<JsString> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let this = this.borrow_mut();
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
  let this = this.borrow_mut();
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
  let this = this.borrow_mut();
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

pub fn get_letterSpacing(mut cx: FunctionContext) -> JsResult<JsString> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let this = this.borrow_mut();
  Ok(cx.string(this.state.letter_spacing.to_string()))
}

pub fn set_letterSpacing(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();

  if let Some(spacing) = opt_spacing_arg(&mut cx, 1)?{
    let em_size = this.state.char_style.font_size();
    this.state.char_style.set_letter_spacing(spacing.in_px(em_size));
    this.state.letter_spacing = spacing;
  }
  Ok(cx.undefined())
}

pub fn get_wordSpacing(mut cx: FunctionContext) -> JsResult<JsString> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let this = this.borrow_mut();
  Ok(cx.string(this.state.word_spacing.to_string()))
}

pub fn set_wordSpacing(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();

  if let Some(spacing) = opt_spacing_arg(&mut cx, 1)?{
    let em_size = this.state.char_style.font_size();
    this.state.char_style.set_word_spacing(spacing.in_px(em_size));
    this.state.word_spacing = spacing;
  }
  Ok(cx.undefined())
}

// -- non-standard typography extensions --------------------------------------------

pub fn get_fontHinting(mut cx: FunctionContext) -> JsResult<JsBoolean> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let this = this.borrow_mut();
  Ok(cx.boolean(this.state.font_hinting))
}

pub fn set_fontHinting(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  let flag = bool_arg(&mut cx, 1, "fontHinting")?;
  this.state.font_hinting = flag;
  Ok(cx.undefined())
}

pub fn get_fontVariant(mut cx: FunctionContext) -> JsResult<JsString> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let this = this.borrow_mut();
  Ok(cx.string(this.state.font_variant.clone()))
}

pub fn set_fontVariant(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  let arg = cx.argument::<JsObject>(1)?;

  let variant = string_for_key(&mut cx, &arg, "variant")?;
  let feat_obj: Handle<JsObject> = arg.get(&mut cx, "features")?;
  let features = font_features(&mut cx, &feat_obj)?;
  this.set_font_variant(&variant, &features);
  Ok(cx.undefined())
}

pub fn get_textWrap(mut cx: FunctionContext) -> JsResult<JsBoolean> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let this = this.borrow_mut();
  Ok(cx.boolean(this.state.text_wrap))
}

pub fn set_textWrap(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  let flag = bool_arg(&mut cx, 1, "textWrap")?;
  this.state.text_wrap = flag;
  Ok(cx.undefined())
}

pub fn get_textDecoration(mut cx: FunctionContext) -> JsResult<JsString> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let this = this.borrow_mut();
  Ok(cx.string(this.state.text_decoration.css.clone()))
}

pub fn set_textDecoration(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  if let Ok(arg) = decoration_arg(&mut cx, 1){
    if let Some(deco_style) = arg{
      let mut this = this.borrow_mut();
      this.state.text_decoration = deco_style;
    }
  }

  Ok(cx.undefined())
}

//
// Effects
//

// -- compositing properties --------------------------------------------------------

pub fn get_globalAlpha(mut cx: FunctionContext) -> JsResult<JsNumber> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let this = this.borrow_mut();
  Ok(cx.number(this.state.global_alpha))
}

pub fn set_globalAlpha(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  let num = float_arg_or_bail(&mut cx, 1, "globalAlpha")?;

  if (0.0..=1.0).contains(&num){
    this.state.global_alpha = num;
  }
  Ok(cx.undefined())
}

pub fn get_globalCompositeOperation(mut cx: FunctionContext) -> JsResult<JsString> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let this = this.borrow_mut();
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
  let this = this.borrow_mut();
  Ok(cx.string(this.state.filter.to_string()))
}

pub fn set_filter(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  if !cx.argument::<JsValue>(1)?.is_a::<JsNull, _>(&mut cx) {
    let (filter_text, specs) = filter_arg(&mut cx, 1)?;
    if filter_text != this.state.filter.to_string() {
      this.state.filter = Filter::new(&filter_text, &specs);
    }
  }
  Ok(cx.undefined())
}

// -- dropshadow properties ---------------------------------------------------------

pub fn get_shadowBlur(mut cx: FunctionContext) -> JsResult<JsNumber> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let this = this.borrow_mut();
  Ok(cx.number(this.state.shadow_blur))
}

pub fn set_shadowBlur(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  let num = float_arg_or_bail(&mut cx, 1, "shadowBlur")?;
  if num >= 0.0{
    this.state.shadow_blur = num;
  }
  Ok(cx.undefined())
}

pub fn get_shadowColor(mut cx: FunctionContext) -> JsResult<JsValue> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let this = this.borrow_mut();
  let shadow_color = this.state.shadow_color;
  color_to_css(&mut cx, &shadow_color)
}

pub fn set_shadowColor(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  if let Some(color) = opt_color_arg(&mut cx, 1){
    this.state.shadow_color = color;
  }
  Ok(cx.undefined())
}

pub fn get_shadowOffsetX(mut cx: FunctionContext) -> JsResult<JsNumber> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let this = this.borrow_mut();
  Ok(cx.number(this.state.shadow_offset.x))
}

pub fn get_shadowOffsetY(mut cx: FunctionContext) -> JsResult<JsNumber> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let this = this.borrow_mut();
  Ok(cx.number(this.state.shadow_offset.y))
}

pub fn set_shadowOffsetX(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  this.state.shadow_offset.x = float_arg_or_bail(&mut cx, 1, "shadowOffsetX")?;
  Ok(cx.undefined())
}

pub fn set_shadowOffsetY(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedContext2D>(0)?;
  let mut this = this.borrow_mut();
  this.state.shadow_offset.y = float_arg_or_bail(&mut cx, 1, "shadowOffsetY")?;
  Ok(cx.undefined())
}
