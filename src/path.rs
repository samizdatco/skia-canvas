#![allow(unused_mut)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(non_snake_case)]
#![allow(dead_code)]
use std::cell::RefCell;
use std::f32::consts::PI;
use neon::prelude::*;
use skia_safe::{Path, Point, PathDirection::{CW, CCW}, Rect, RRect, Matrix, PathOp, StrokeRec,};
use skia_safe::{PathEffect, trim_path_effect};
use skia_safe::path::{self, AddPathMode, Verb, FillType};

use crate::utils::*;

pub type BoxedPath2D = JsBox<RefCell<Path2D>>;
impl Finalize for Path2D {}

pub struct Path2D{
  pub path:Path
}

impl Default for Path2D {
  fn default() -> Self {
    Self{ path:Path::new() }
  }
}

impl Path2D{
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
  let path = Path::from_svg(svg_string).unwrap_or_default();
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
  check_argc(&mut cx, 3)?;

  let xy = opt_float_args(&mut cx, 1..3);
  if let [x, y] = xy.as_slice(){
    this.path.move_to((*x, *y));
  }

  Ok(cx.undefined())
}

// Connects the last point in the subpath to the (x, y) coordinates with a straight line.
pub fn lineTo(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedPath2D>(0)?;
  let mut this = this.borrow_mut();
  check_argc(&mut cx, 3)?;

  let xy = opt_float_args(&mut cx, 1..3);
  if let [x, y] = xy.as_slice(){
    this.path.line_to((*x, *y));
  }
  Ok(cx.undefined())
}

// Adds a cubic Bézier curve to the path. It requires three points. The first two points are control points and the third one is the end point. The starting point is the last point in the current path, which can be changed using moveTo() before creating the Bézier curve.
pub fn bezierCurveTo(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedPath2D>(0)?;
  let mut this = this.borrow_mut();
  check_argc(&mut cx, 7)?;

  let nums = opt_float_args(&mut cx, 1..7);
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
  check_argc(&mut cx, 5)?;

  let nums = opt_float_args(&mut cx, 1..5);
  if let [cpx, cpy, x, y] = nums.as_slice(){
    this.scoot(*cpx, *cpy);
    this.path.quad_to((*cpx, *cpy), (*x, *y));
  }

  Ok(cx.undefined())
}

// Adds a conic-section curve to the current path.
pub fn conicCurveTo(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedPath2D>(0)?;
  let mut this = this.borrow_mut();
  check_argc(&mut cx, 6)?;

  let nums = opt_float_args(&mut cx, 1..6);
  if let [p1x, p1y, p2x, p2y, weight] = nums.as_slice(){
    this.scoot(*p1x, *p1y);
    this.path.conic_to((*p1x, *p1y), (*p2x, *p2y), *weight);
  }

  Ok(cx.undefined())
}

// Adds an arc to the path which is centered at (x, y) position with radius r starting at startAngle and ending at endAngle going in the given direction by anticlockwise (defaulting to clockwise).
pub fn arc(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedPath2D>(0)?;
  let mut this = this.borrow_mut();
  check_argc(&mut cx, 6)?;

  let nums = opt_float_args(&mut cx, 1..6);
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
  check_argc(&mut cx, 6)?;

  let nums = opt_float_args(&mut cx, 1..6);
  if let [x1, y1, x2, y2, radius] = nums.as_slice(){
    this.scoot(*x1, *y1);
    this.path.arc_to_tangent((*x1, *y1), (*x2, *y2), *radius);
  }

  Ok(cx.undefined())
}

// Adds an elliptical arc to the path which is centered at (x, y) position with the radii radiusX and radiusY starting at startAngle and ending at endAngle going in the given direction by anticlockwise (defaulting to clockwise).
pub fn ellipse(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedPath2D>(0)?;
  let mut this = this.borrow_mut();
  check_argc(&mut cx, 8)?;

  let nums = opt_float_args(&mut cx, 1..8);
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
  check_argc(&mut cx, 5)?;

  let nums = opt_float_args(&mut cx, 1..5);
  if let [x, y, w, h] = nums.as_slice(){
    let rect = Rect::from_xywh(*x, *y, *w, *h);
    let direction = if w.signum() == h.signum(){ CW }else{ CCW };
    this.path.add_rect(rect, Some((direction, 0)));
  }

  Ok(cx.undefined())
}

// Creates a path for a rounded rectangle at position (x, y) with a size (w, h) and whose radii
// are specified in x/y pairs for top_left, top_right, bottom_right, and bottom_left
pub fn roundRect(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedPath2D>(0)?;
  let mut this = this.borrow_mut();
  check_argc(&mut cx, 13)?;

  let nums = opt_float_args(&mut cx, 1..13);
  if let [x, y, w, h] = &nums[..4]{
    let rect = Rect::from_xywh(*x, *y, *w, *h);
    let radii:Vec<Point> = nums[4..].chunks(2).map(|xy| Point::new(xy[0], xy[1])).collect();
    let rrect = RRect::new_rect_radii(rect, &[radii[0], radii[1], radii[2], radii[3]]);
    let direction = if w.signum() == h.signum(){ CW }else{ CCW };
    this.path.add_rrect(rrect, Some((direction, 0)));
  }

  Ok(cx.undefined())
}

// Applies a boolean operator to this and a second path, returning a new Path2D with their combination
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

pub fn interpolate(mut cx: FunctionContext) -> JsResult<BoxedPath2D> {
  let this = cx.argument::<BoxedPath2D>(0)?;
  let other = cx.argument::<BoxedPath2D>(1)?;
  let weight = float_arg(&mut cx, 2, "weight")?;

  let this = this.borrow();
  let other = other.borrow();
  // reverse path order since 0..1 = this..other is a less non-sensical mapping than the default
  if let Some(path) = other.path.interpolate(&this.path, weight){
    Ok(cx.boxed(RefCell::new(Path2D{ path })))
  }else{
    cx.throw_type_error("Can only interpolate between two Path2D objects with the same number of points and control points")
  }
}

// Returns a path with only non-overlapping contours that describe the same area as the original path
pub fn simplify(mut cx: FunctionContext) -> JsResult<BoxedPath2D> {
  let this = cx.argument::<BoxedPath2D>(0)?;
  let rule = fill_rule_arg_or(&mut cx, 1, "nonzero")?;
  let mut this = this.borrow_mut();

  this.path.set_fill_type(rule);

  let new_path = Path2D{
    path:match this.path.simplify(){
      Some(simpler) => simpler,
      None => this.path.clone()
    }
  };

  Ok(cx.boxed(RefCell::new(new_path)))
}

// Returns a path that can be drawn with a nonzero fill but looks like the original drawn with evenodd
pub fn unwind(mut cx: FunctionContext) -> JsResult<BoxedPath2D> {
  let this = cx.argument::<BoxedPath2D>(0)?;
  let mut this = this.borrow_mut();

  this.path.set_fill_type(FillType::EvenOdd);

  let new_path = Path2D{
    path:match this.path.as_winding(){
      Some(rewound) => rewound,
      None => this.path.clone()
    }
  };

  Ok(cx.boxed(RefCell::new(new_path)))
}

// Returns a copy whose points have been shifted by (dx, dy)
pub fn offset(mut cx: FunctionContext) -> JsResult<BoxedPath2D> {
  let this = cx.argument::<BoxedPath2D>(0)?;
  let dx = float_arg(&mut cx, 1, "dx")?;
  let dy = float_arg(&mut cx, 2, "dy")?;

  let this = this.borrow();
  let path = this.path.with_offset((dx, dy));
  Ok(cx.boxed(RefCell::new(Path2D{path})))
}

// Returns a copy whose points have been transformed by a given matrix
pub fn transform(mut cx: FunctionContext) -> JsResult<BoxedPath2D> {
  let this = cx.argument::<BoxedPath2D>(0)?;
  let matrix = matrix_arg(&mut cx, 1)?;

  let this = this.borrow();
  let path = this.path.with_transform(&matrix);
  Ok(cx.boxed(RefCell::new(Path2D{path})))
}

// Returns a copy where every sharp junction to an arcTo-style rounded corner
pub fn round(mut cx: FunctionContext) -> JsResult<BoxedPath2D> {
  let this = cx.argument::<BoxedPath2D>(0)?;
  let radius = float_arg(&mut cx, 1, "radius")?;

  let this = this.borrow();
  let bounds = this.path.bounds();
  let stroke_rec = StrokeRec::new_hairline();

  if let Some(rounder) = PathEffect::corner_path(radius){
    if let Some((path, _)) = rounder.filter_path(&this.path, &stroke_rec, bounds){
      return Ok(cx.boxed(RefCell::new(Path2D{path})))
    }
  }

  Ok(cx.boxed(RefCell::new(Path2D{path:this.path.clone()})))
}

// Clips a proportional segment out of the middle of the path (or the edges if invert=true)
pub fn trim(mut cx: FunctionContext) -> JsResult<BoxedPath2D> {
  let this = cx.argument::<BoxedPath2D>(0)?;
  let begin = float_arg(&mut cx, 1, "begin")?;
  let end = float_arg(&mut cx, 2, "end")?;
  let invert = bool_arg_or(&mut cx, 3, false);

  let this = this.borrow();
  let bounds = this.path.bounds();
  let stroke_rec = StrokeRec::new_hairline();
  let mode = if invert{ trim_path_effect::Mode::Inverted }else{ trim_path_effect::Mode::Normal };

  if let Some(trimmer) = PathEffect::trim(begin, end, mode){
    if let Some((path, _)) = trimmer.filter_path(&this.path, &stroke_rec, bounds){
      return Ok(cx.boxed(RefCell::new(Path2D{path})))
    }
  }

  Ok(cx.boxed(RefCell::new(Path2D{path:this.path.clone()})))
}

// Discretizes the path at a fixed segment length then randomly offsets the points
pub fn jitter(mut cx: FunctionContext) -> JsResult<BoxedPath2D> {
  let this = cx.argument::<BoxedPath2D>(0)?;
  let seg_len = float_arg(&mut cx, 1, "segmentLength")?;
  let std_dev = float_arg(&mut cx, 2, "variance")?;
  let seed = float_arg_or(&mut cx, 3, 0.0) as u32;

  let this = this.borrow();
  let bounds = this.path.bounds();
  let stroke_rec = StrokeRec::new_hairline();

  if let Some(trimmer) = PathEffect::discrete(seg_len, std_dev, Some(seed)){
    if let Some((path, _)) = trimmer.filter_path(&this.path, &stroke_rec, bounds){
      return Ok(cx.boxed(RefCell::new(Path2D{path})))
    }
  }

  Ok(cx.boxed(RefCell::new(Path2D{path:this.path.clone()})))
}

// Returns the computed `tight` bounds that contain all the points, control points, and connecting contours
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

pub fn contains(mut cx: FunctionContext) -> JsResult<JsBoolean> {
  let this = cx.argument::<BoxedPath2D>(0)?;
  let x = float_arg(&mut cx, 1, "x")?;
  let y = float_arg(&mut cx, 2, "y")?;
  let this = this.borrow();

  Ok(cx.boolean(this.path.contains((x,y))))
}

fn from_verb(verb:Verb) -> Option<String>{
  let cmd = match verb{
    Verb::Move => "moveTo",
    Verb::Line => "lineTo",
    Verb::Quad => "quadraticCurveTo",
    Verb::Cubic => "bezierCurveTo",
    Verb::Conic => "conicCurveTo",
    Verb::Close => "closePath",
    _ => return None
  };
  Some(cmd.to_string())
}

pub fn edges(mut cx: FunctionContext) -> JsResult<JsArray> {
  let this = cx.argument::<BoxedPath2D>(0)?;
  let this = this.borrow();

  let mut weights = path::Iter::new(&this.path, false);
  let iter = path::Iter::new(&this.path, false);

  let mut edges = vec![];
  for (verb, points) in iter{
    weights.next();

    if let Some(edge) = from_verb(verb){
      let cmd = cx.string(edge);
      let segment = JsArray::new(&mut cx, 1 + points.len() as u32);
      segment.set(&mut cx, 0, cmd)?;

      let at_point = if points.len()>1{ 1 }else{ 0 };
      for (i, pt) in points.iter().skip(at_point).enumerate() {
        let x = cx.number(pt.x);
        let y = cx.number(pt.y);
        segment.set(&mut cx, 1 + 2*i as u32, x)?;
        segment.set(&mut cx, 2 + 2*i as u32, y)?;
      }

      if verb==Verb::Conic{
        let weight = weights.conic_weight().unwrap();
        let weight = cx.number(weight);
        segment.set(&mut cx, 5, weight)?;
      }

      edges.push(segment);
    }
  }

  let verbs = JsArray::new(&mut cx, edges.len() as u32);
  for (i, segment) in edges.iter().enumerate(){
    verbs.set(&mut cx, i as u32, *segment)?;
  }

  Ok(verbs)
}

pub fn get_d(mut cx: FunctionContext) -> JsResult<JsString> {
  let this = cx.argument::<BoxedPath2D>(0)?;
  let this = this.borrow();
  Ok(cx.string(this.path.to_svg()))
}

pub fn set_d(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedPath2D>(0)?;
  let svg_string = string_arg(&mut cx, 1, "svgPath")?;
  let mut this = this.borrow_mut();

  if let Some(path) = Path::from_svg(svg_string){
    this.path.rewind();
    this.path.add_path(&path, (0,0), None);
    Ok(cx.undefined())
  }else{
    cx.throw_type_error("Expected a valid SVG path string")
  }
}