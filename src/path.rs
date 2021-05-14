#![allow(unused_mut)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(non_snake_case)]
#![allow(dead_code)]
use std::cell::RefCell;
use std::f32::consts::PI;
use neon::prelude::*;
use skia_safe::{Path, Point, PathDirection, Rect, Matrix, PathOp};
use skia_safe::path::{AddPathMode};

use crate::utils::*;

pub type BoxedPath2D = JsBox<RefCell<Path2D>>;
impl Finalize for Path2D {}

pub struct Path2D{
  pub path:Path
}

impl Path2D{
  pub fn new() -> Self{
    Self{ path:Path::new() }
  }

  pub fn scoot(&mut self, x: f32, y: f32){
    if self.path.is_empty(){
      self.path.move_to((x, y));
    }
  }

  pub fn add_ellipse(&mut self, origin:impl Into<Point>, radii:impl Into<Point>, rotation: f32, start_angle:f32, end_angle:f32, ccw:bool){
    let Point{x, y} = origin.into();
    let Point{x:x_radius, y:y_radius} = radii.into();

    // based off of CanonicalizeAngle in Chrome
    let tau = 2.0 * PI;
    let mut new_start_angle = start_angle % tau;
    if new_start_angle < 0.0 {
      new_start_angle += tau;
    }
    let delta = new_start_angle - start_angle;
    let start_angle = new_start_angle;
    let mut end_angle = end_angle + delta;

    // Based off of AdjustEndAngle in Chrome.
    if !ccw && (end_angle - start_angle) >= tau {
      end_angle = start_angle + tau; // Draw complete ellipse
    } else if ccw && (start_angle - end_angle) >= tau {
      end_angle = start_angle - tau; // Draw complete ellipse
    } else if !ccw && start_angle > end_angle {
      end_angle = start_angle + (tau - (start_angle - end_angle) % tau);
    } else if ccw && start_angle < end_angle {
      end_angle = start_angle - (tau - (end_angle - start_angle) % tau);
    }

    // Based off of Chrome's implementation in
    // https://cs.chromium.org/chromium/src/third_party/blink/renderer/platform/graphics/path.cc
    // of note, can't use addArc or addOval because they close the arc, which
    // the spec says not to do (unless the user explicitly calls closePath).
    // This throws off points being in/out of the arc.
    let oval = Rect::new(x - x_radius, y - y_radius, x + x_radius, y + y_radius);
    let mut rotated = Matrix::new_identity();
    rotated
      .pre_translate((x, y))
      .pre_rotate(to_degrees(rotation), None)
      .pre_translate((-x, -y));
    let unrotated = rotated.invert().unwrap();

    self.path.transform(&unrotated);

    // draw in 2 180 degree segments because trying to draw all 360 degrees at once
    // draws nothing.
    let sweep_deg = to_degrees(end_angle - start_angle);
    let start_deg = to_degrees(start_angle);
    if almost_equal(sweep_deg.abs(), 360.0) {
      let half_sweep = sweep_deg/2.0;
      self.path.arc_to(oval, start_deg, half_sweep, false);
      self.path.arc_to(oval, start_deg + half_sweep, half_sweep, false);
    }else{
      self.path.arc_to(oval, start_deg, sweep_deg, false);
    }

    self.path.transform(&rotated);
  }
}

//
// -- Javascript Methods --------------------------------------------------------------------------
//

pub fn new(mut cx: FunctionContext) -> JsResult<BoxedPath2D> {
  let path = Path::new();
  Ok(cx.boxed(RefCell::new(Path2D{path})))
}

pub fn from_path(mut cx: FunctionContext) -> JsResult<BoxedPath2D> {
  let other_path = cx.argument::<BoxedPath2D>(1)?;
  let path = other_path.borrow().path.clone();
  Ok(cx.boxed(RefCell::new(Path2D{path})))
}

pub fn from_svg(mut cx: FunctionContext) -> JsResult<BoxedPath2D> {
  let svg_string = string_arg(&mut cx, 1, "svgPath")?;
  let path = Path::from_svg(svg_string).unwrap_or_else(Path::new);
  Ok(cx.boxed(RefCell::new(Path2D{path})))
}

// Adds a path to the current path.
pub fn addPath(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedPath2D>(0)?;
  let other = cx.argument::<BoxedPath2D>(1)?;
  let matrix = opt_matrix_arg(&mut cx, 2).unwrap_or_else(
    Matrix::new_identity
  );

  // make a copy if adding a path to itself, otherwise use a ref
  if this.strict_equals(&mut cx, other){
    let src = other.borrow().path.clone();
    let mut dst = &mut this.borrow_mut().path;
    dst.add_path_matrix(&src, &matrix, AddPathMode::Append);
  }else{
    let src = &other.borrow().path;
    let mut dst = &mut this.borrow_mut().path;
    dst.add_path_matrix(src, &matrix, AddPathMode::Append);
  };

  Ok(cx.undefined())
}

// Causes the point of the pen to move back to the start of the current sub-path. It tries to draw a straight line from the current point to the start. If the shape has already been closed or has only one point, this function does nothing.
pub fn closePath(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedPath2D>(0)?;
  let mut this = this.borrow_mut();
  this.path.close();
  Ok(cx.undefined())
}

// Moves the starting point of a new sub-path to the (x, y) coordinates.
pub fn moveTo(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedPath2D>(0)?;
  let mut this = this.borrow_mut();
  let x = float_arg(&mut cx, 1, "x")?;
  let y = float_arg(&mut cx, 2, "y")?;

  this.path.move_to((x, y));
  Ok(cx.undefined())
}

// Connects the last point in the subpath to the (x, y) coordinates with a straight line.
pub fn lineTo(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedPath2D>(0)?;
  let mut this = this.borrow_mut();
  let x = float_arg(&mut cx, 1, "x")?;
  let y = float_arg(&mut cx, 2, "y")?;

  this.scoot(x, y);
  this.path.line_to((x, y));
  Ok(cx.undefined())
}

// Adds a cubic Bézier curve to the path. It requires three points. The first two points are control points and the third one is the end point. The starting point is the last point in the current path, which can be changed using moveTo() before creating the Bézier curve.
pub fn bezierCurveTo(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedPath2D>(0)?;
  let mut this = this.borrow_mut();
  let nums = float_args(&mut cx, 1..7)?;
  if let [cp1x, cp1y, cp2x, cp2y, x, y] = nums.as_slice(){
    this.scoot(*cp1x, *cp1y);
    this.path.cubic_to((*cp1x, *cp1y), (*cp2x, *cp2y), (*x, *y));
  }

  Ok(cx.undefined())
}

// Adds a quadratic Bézier curve to the current path.
pub fn quadraticCurveTo(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedPath2D>(0)?;
  let mut this = this.borrow_mut();
  let nums = float_args(&mut cx, 1..5)?;
  if let [cpx, cpy, x, y] = nums.as_slice(){
    this.scoot(*cpx, *cpy);
    this.path.quad_to((*cpx, *cpy), (*x, *y));
  }

  Ok(cx.undefined())
}

// Adds an arc to the path which is centered at (x, y) position with radius r starting at startAngle and ending at endAngle going in the given direction by anticlockwise (defaulting to clockwise).
pub fn arc(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedPath2D>(0)?;
  let mut this = this.borrow_mut();
  let nums = float_args(&mut cx, 1..6)?;
  let ccw = bool_arg_or(&mut cx, 6, false);

  if let [x, y, radius, start_angle, end_angle] = nums.as_slice(){
    this.add_ellipse((*x, *y), (*radius, *radius), 0.0, *start_angle, *end_angle, ccw);
  }

  Ok(cx.undefined())
}

// Adds a circular arc to the path with the given control points and radius, connected to the previous point by a straight line.
pub fn arcTo(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedPath2D>(0)?;
  let mut this = this.borrow_mut();
  let coords = float_args(&mut cx, 1..5)?;
  let radius = float_arg(&mut cx, 5, "radius")?;

  if let [x1, y1, x2, y2] = coords.as_slice(){
    this.scoot(*x1, *y1);
    this.path.arc_to_tangent((*x1, *y1), (*x2, *y2), radius);
  }

  Ok(cx.undefined())
}

// Adds an elliptical arc to the path which is centered at (x, y) position with the radii radiusX and radiusY starting at startAngle and ending at endAngle going in the given direction by anticlockwise (defaulting to clockwise).
pub fn ellipse(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedPath2D>(0)?;
  let mut this = this.borrow_mut();
  let nums = float_args(&mut cx, 1..8)?;
  let ccw = bool_arg_or(&mut cx, 8, false);

  if let [x, y, x_radius, y_radius, rotation, start_angle, end_angle] = nums.as_slice(){
    if *x_radius < 0.0 || *y_radius < 0.0 {
      return cx.throw_error("radii cannot be negative")
    }

    this.add_ellipse((*x, *y), (*x_radius, *y_radius), *rotation, *start_angle, *end_angle, ccw);
  }

  Ok(cx.undefined())
}

// Creates a path for a rectangle at position (x, y) with a size that is determined by width and height.
pub fn rect(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedPath2D>(0)?;
  let mut this = this.borrow_mut();
  let nums = float_args(&mut cx, 1..5)?;

  if let [x, y, w, h] = nums.as_slice(){
    let rect = Rect::from_xywh(*x, *y, *w, *h);
    this.path.add_rect(rect, Some((PathDirection::CW, 0)));
  }

  Ok(cx.undefined())
}

pub fn op(mut cx: FunctionContext) -> JsResult<BoxedPath2D> {
  let this = cx.argument::<BoxedPath2D>(0)?;
  let other_path = cx.argument::<BoxedPath2D>(1)?;
  let op_name = string_arg(&mut cx, 2, "pathOp")?;

  if let Some(path_op) = to_path_op(&op_name){
    let this = this.borrow();
    let other = other_path.borrow();
    match this.path.op(&other.path, path_op) {
      Some(path) => Ok(cx.boxed(RefCell::new(Path2D{ path }))),
      None => cx.throw_error("path operation failed")
    }
  }else{
    cx.throw_error("pathOp must be Difference, Intersect, Union, XOR, or Complement")
  }
}

pub fn simplify(mut cx: FunctionContext) -> JsResult<BoxedPath2D> {
  let this = cx.argument::<BoxedPath2D>(0)?;
  let this = this.borrow();

  let new_path = Path2D{
    path:match this.path.simplify(){
      Some(simpler) => simpler,
      None => this.path.clone()
    }
  };

  Ok(cx.boxed(RefCell::new(new_path)))
}

pub fn bounds(mut cx: FunctionContext) -> JsResult<JsObject> {
  let this = cx.argument::<BoxedPath2D>(0)?;
  let this = this.borrow();

  let b = match this.path.tight_bounds(){
    Some(rect) => rect,
    None => this.path.compute_tight_bounds()
  };

  let js_object: Handle<JsObject> = cx.empty_object();
  let left = cx.number(b.left);
  let top = cx.number(b.top);
  let right = cx.number(b.right);
  let bottom = cx.number(b.bottom);
  let width = cx.number(b.width());
  let height = cx.number(b.height());

  js_object.set(&mut cx, "left", left)?;
  js_object.set(&mut cx, "top", top)?;
  js_object.set(&mut cx, "right", right)?;
  js_object.set(&mut cx, "bottom", bottom)?;
  js_object.set(&mut cx, "width", width)?;
  js_object.set(&mut cx, "height", height)?;
  Ok(js_object)
}
