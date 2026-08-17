#![allow(unused_mut)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(non_snake_case)]
use std::cell::{Ref, RefCell};
use std::f32::EPSILON;
use std::f64::consts::{FRAC_PI_2, PI, TAU};
use neon::prelude::*;
use skia_safe::{Path, Point, Vector, PathFillType, PathDirection, PathBuilder, Rect, RRect, Matrix, PathOp, StrokeRec};
use skia_safe::{PathEffect, trim_path_effect, ContourMeasure, ContourMeasureIter};
use skia_safe::path::{self, AddPathMode, Verb};

use crate::bridge::*;
use crate::drawlist::Pen;
use crate::mem;

pub type BoxedPath2D = JsBox<RefCell<Path2D>>;
impl Finalize for Path2D {}

pub struct Path2D{
  builder: PathBuilder,
  cache: RefCell<Option<Path>>, // lazy snapshot of the builder; invalidated on every append
  measure: RefCell<Option<Measure>>, // lazy whole-path arc-length ruler; invalidated with the cache
  footprint: RefCell<mem::v8::Footprint>, // retained geometry & measure size reported to V8
}

impl Default for Path2D {
  fn default() -> Self {
    Self{
      builder:PathBuilder::new(), cache:RefCell::new(None), measure:RefCell::new(None),
      footprint:RefCell::new(mem::v8::Footprint::default())
    }
  }
}

impl From<PathBuilder> for Path2D{
  fn from(builder: PathBuilder) -> Self {
    // leave the cache empty: the geometry is charged when `path()` first materializes it
    Self{ builder, ..Self::default() }
  }
}

impl From<Path> for Path2D{
  fn from(path: Path) -> Self {
    let footprint = mem::v8::Footprint::new(path.approximate_bytes_used());
    Self{
      builder:PathBuilder::new_path(&path), cache:RefCell::new(Some(path)),
      footprint:RefCell::new(footprint), ..Self::default()
    }
  }
}

impl Path2D{
  // either return the cached path (if no additional appends have happened) or replay the builder to generate a new one
  pub fn path(&self) -> Path {
    let mut cache = self.cache.borrow_mut();
    if cache.is_none() {
      let snapshot = self.builder.snapshot();
      self.footprint.borrow_mut().set(snapshot.approximate_bytes_used());
      *cache = Some(snapshot);
    }
    cache.as_ref().unwrap().clone()
  }

  // generate a fresh path from the geometry with the transform baked into it
  pub fn snapshot_transformed(&self, matrix: &Matrix) -> Path {
    self.builder.snapshot_and_transform(Some(matrix))
  }

  // invalidate the cache and return the builder (to allow for a new append)
  pub fn update(&mut self) -> &mut PathBuilder {
    self.cache.borrow_mut().take();
    if let Some(measure) = self.measure.borrow_mut().take(){
      self.footprint.borrow_mut().grow(-(measure.bytes as i64));
    }
    &mut self.builder
  }

  // borrow the arc-length ruler (and lazily build it if necessary)
  pub fn measure(&self) -> Ref<'_, Measure> {
    if self.measure.borrow().is_none(){
      let ruler = Measure::new(&self.path());
      self.footprint.borrow_mut().grow(ruler.bytes as i64);
      *self.measure.borrow_mut() = Some(ruler);
    }
    Ref::map(self.measure.borrow(), |measure| measure.as_ref().unwrap())
  }

  pub fn scoot(&mut self, x: f32, y: f32){
    if self.builder.is_empty(){
      self.update().move_to((x, y));
    }
  }

  // the current point (end of the last-added verb), or None if the path is empty
  pub fn last_point(&self) -> Option<Point> {
    self.path().last_pt()
  }

  pub fn append_path(&mut self, src:&Path, matrix:&Matrix){
    self.update().add_path_with_transform(src, matrix, AddPathMode::Append);
  }

  pub fn extend_path(&mut self, sub:&Path, matrix:&Matrix){
    self.update().add_path_with_transform(sub, matrix, AddPathMode::Extend);
  }

  pub fn conic_to_normalized(&mut self, p1:impl Into<Point>, p2:impl Into<Point>, weight:f32){
    let (p1, p2) = (p1.into(), p2.into());
    let builder = self.update();

    // PathBuilder::conic_to doesn't normalize weights, so do some special casing for nonsense values
    if !(weight > 0.0){
      builder.line_to(p2);
    }else if !weight.is_finite(){
      builder.line_to(p1);
      builder.line_to(p2);
    }else if weight == 1.0{
      builder.quad_to(p1, p2);
    }else{
      builder.conic_to(p1, p2, weight);
    }
  }

  pub fn add_ellipse(&mut self, origin:impl Into<Point>, radii:impl Into<Point>, rotation: f64, start_angle:f64, end_angle:f64, ccw:bool){
    let Point{x, y} = origin.into();
    let Point{x:x_radius, y:y_radius} = radii.into();

    let tau = 2.0 * std::f64::consts::PI;
    let mut new_start_angle = start_angle % tau;
    if new_start_angle < 0.0 {
      new_start_angle += tau;
    }
    let delta = new_start_angle - start_angle;
    let start_angle = new_start_angle;
    let mut end_angle = end_angle + delta;

    if !ccw && start_angle > end_angle {
      end_angle = start_angle + (tau - (start_angle - end_angle) % tau);
    }else if ccw && start_angle < end_angle {
      end_angle = start_angle - (tau - (end_angle - start_angle) % tau);
    }

    let oval = Rect::new(x - x_radius, y - y_radius, x + x_radius, y + y_radius);

    let mut rotated = Matrix::new_identity();
    rotated
      .pre_translate((x, y))
      .pre_rotate(rotation.to_degrees() as f32, None)
      .pre_translate((-x, -y));

    // build the arc independently then *extend* the path (drawing a connecting line from the prior point)
    let mut arc = PathBuilder::new();
    {
      let sweep_deg = (end_angle - start_angle).to_degrees();
      let start_deg = start_angle.to_degrees() % 360.0;

      // draw 360° ellipses in two 180° segments; trying to draw the full ellipse at once draws nothing
      if sweep_deg >= 360.0 - EPSILON as f64 {
        arc.arc_to(oval, start_deg as f32, 180.0, false);
        arc.arc_to(oval, ((start_deg + 180.0) % 360.0) as f32, 180.0, false);
      }else if sweep_deg <= -360.0 + EPSILON as f64 {
        arc.arc_to(oval, start_deg as f32, -180.0, false);
        arc.arc_to(oval, ((start_deg - 180.0) % 360.0) as f32, -180.0, false);
      }else{
        // Draw <360° ellipses in a single arc
        arc.arc_to(oval, start_deg as f32, sweep_deg as f32, false);
      }
    }
    self.extend_path(&arc.detach(), &rotated);
  }
}

// receives verbs from the shared DrawList decoder and applies them verbatim (i.e., no baked-in CTM)
impl Pen for Path2D {
  fn move_to(&mut self, x:f32, y:f32){ self.update().move_to((x, y)); }
  fn line_to(&mut self, x:f32, y:f32){ self.scoot(x, y); self.update().line_to((x, y)); }
  fn bezier_to(&mut self, c1x:f32, c1y:f32, c2x:f32, c2y:f32, x:f32, y:f32){
    self.scoot(c1x, c1y);
    self.update().cubic_to((c1x, c1y), (c2x, c2y), (x, y));
  }
  fn quad_to(&mut self, cx:f32, cy:f32, x:f32, y:f32){
    self.scoot(cx, cy);
    self.update().quad_to((cx, cy), (x, y));
  }
  fn conic_to(&mut self, cx:f32, cy:f32, x:f32, y:f32, w:f32){
    self.scoot(cx, cy);
    self.conic_to_normalized((cx, cy), (x, y), w);
  }
  fn arc(&mut self, x:f32, y:f32, r:f32, start:f64, end:f64, ccw:bool){
    self.add_ellipse((x, y), (r, r), 0.0, start, end, ccw);
  }
  fn arc_to(&mut self, x1:f32, y1:f32, x2:f32, y2:f32, r:f32){
    self.scoot(x1, y1);
    self.update().arc_to_tangent((x1, y1), (x2, y2), r);
  }
  fn ellipse(&mut self, x:f32, y:f32, xr:f32, yr:f32, rot:f64, start:f64, end:f64, ccw:bool){
    self.add_ellipse((x, y), (xr, yr), rot, start, end, ccw);
  }
  fn rect(&mut self, x:f32, y:f32, w:f32, h:f32){
    // enulate browser's corner orderL moveTo(x,y), then CW/CCW based on the sign of w×h
    self.update()
      .move_to((x, y))
      .line_to((x+w, y))
      .line_to((x+w, y+h))
      .line_to((x, y+h))
      .close();
  }
  fn round_rect(&mut self, x:f32, y:f32, w:f32, h:f32, radii:[Point;4]){
    let rect = Rect::from_xywh(x, y, w, h);
    let rrect = RRect::new_rect_radii(rect, &radii);
    let direction = if w.signum() == h.signum(){ PathDirection::CW }else{ PathDirection::CCW };
    self.update().add_rrect(rrect, direction, 0);
  }
  fn close(&mut self){ self.update().close(); }
}

// measure all the path's contours laid end-to-end (so a distance can locate any point on the path)
pub struct Measure{
  contours: Vec<ContourMeasure>,
  offsets: Vec<f64>, // cumulative start distance of each contour
  total: f64, // total length of the whole path (f64, so accumulation doesn't drift on huge paths)
  bytes: usize, // estimated size of the segment tables
}

impl Measure{
  fn new(path:&Path) -> Self {
    let mut contours = vec![];
    let mut offsets = vec![];
    let mut total = 0.0;
    // force_closed:false — don't invent a closing segment for open contours
    // (zero-length contours are skipped by the iterator entirely)
    for contour in ContourMeasureIter::new(path, false, None){
      offsets.push(total);
      total += contour.length() as f64;
      contours.push(contour);
    }
    Self{contours, offsets, total, bytes:measure_size_estimate(path)}
  }

  // map a whole-path distance to a position + tangent
  fn locate(&self, d:f64) -> Option<(Point, Vector)> {
    if d.is_nan(){ return None } // bail on NaN
    let d = d.clamp(0.0, self.total); // clamp out-of-bounds distances into range
    let idx = self.offsets.partition_point(|&start| start <= d).saturating_sub(1);
    let contour = self.contours.get(idx)?;
    contour.pos_tan((d - self.offsets[idx]) as f32)
  }

  // the tangent at a given distance (tangent is f32s but converted to f64 for the atan2)
  fn heading(&self, d:f64) -> Option<f64> {
    self.locate(d).map(|(_, tangent)| (tangent.y as f64).atan2(tangent.x as f64))
  }
}

// estimate the size of the measure table based on Skia's curve-subdivision behavior
fn measure_size_estimate(path:&Path) -> usize {
  const TOL:f32 = 0.5; // CHEAP_DIST_LIMIT / res_scale
  let dev = |p:Point, q:Point| (p.x - q.x).abs().max((p.y - q.y).abs());
  let lerp = |p:Point, q:Point, t:f32| Point::new(p.x + (q.x - p.x) * t, p.y + (q.y - p.y) * t);
  let curve_segs = |deviation:f32| {
    let levels = (deviation / TOL).max(1.0).log2() / 2.0; // log₄ of the deviation ratio
    1usize << (levels.ceil().min(8.0) as u32) // kMaxRecursionDepth = 8 → ≤256 segments
  };

  let (mut segments, mut points) = (0usize, 0usize);
  for (verb, pts) in path::Iter::new(path, false){
    let (segs, n_pts) = match verb{
      Verb::Move => (0, 1),
      Verb::Line | Verb::Close => (1, 1), // close emits a line back to the contour's start
      Verb::Quad => (curve_segs(0.5 * dev(pts[1], lerp(pts[0], pts[2], 0.5))), 2),
      Verb::Conic => (curve_segs(0.5 * dev(pts[1], lerp(pts[0], pts[2], 0.5))), 3), // the weight rides in an extra point
      Verb::Cubic => (curve_segs(
        dev(pts[1], lerp(pts[0], pts[3], 1.0/3.0)).max(dev(pts[2], lerp(pts[0], pts[3], 2.0/3.0)))
      ), 3),
      _ => (0, 0),
    };
    segments += segs;
    points += n_pts;
  }
  12 * segments + 8 * points // sizeof(Segment) and sizeof(SkPoint)
}


//
// -- Javascript Methods --------------------------------------------------------------------------
//

pub fn new(mut cx: FunctionContext) -> JsResult<BoxedPath2D> {
  Ok(cx.boxed(RefCell::new(Path2D::default())))
}

pub fn from_path(mut cx: FunctionContext) -> JsResult<BoxedPath2D> {
  let other_path = path2d_arg(&mut cx, 1)?;
  let path = other_path.borrow().path();
  Ok(cx.boxed(RefCell::new(Path2D::from(path))))
}

pub fn from_svg(mut cx: FunctionContext) -> JsResult<BoxedPath2D> {
  let svg_string = string_arg(&mut cx, 1, "svgPath")?;
  let path = Path::from_svg(svg_string).unwrap_or_default();
  Ok(cx.boxed(RefCell::new(Path2D::from(path))))
}

// Adds a path to the current path.
pub fn addPath(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedPath2D>(0)?;
  let other = path2d_arg(&mut cx, 1)?;
  let matrix = opt_matrix_arg(&mut cx, 2).unwrap_or_else(
    Matrix::new_identity
  );

  // the snapshot is already a copy, so this is safe even when adding a path to itself
  let src = other.borrow().path();
  this.borrow_mut().append_path(&src, &matrix);

  Ok(cx.undefined())
}

// convert skia's path-op results (which all expect to be drawn with `even-odd` winding) so they'll
// work correctly in the `nonzero` world of canvas's Path2D
fn rule_independent(path:Path) -> Path {
  let mut path = path;
  path.set_fill_type(PathFillType::EvenOdd);
  let mut path = match path.as_winding(){ Some(rewound) => rewound, None => path };
  path.set_fill_type(PathFillType::EvenOdd);
  match path.simplify(){ Some(simpler) => simpler, None => path }
}

// Applies a boolean operator to this and a second path, returning a new Path2D with their combination
pub fn op(mut cx: FunctionContext) -> JsResult<BoxedPath2D> {
  let this = cx.argument::<BoxedPath2D>(0)?;
  let other_path = path2d_arg(&mut cx, 1)?;
  let op_name = string_arg(&mut cx, 2, "pathOp")?;

  if let Some(path_op) = to_path_op(&op_name){
    let this = this.borrow();
    let other = other_path.borrow();
    match this.path().op(&other.path(), path_op) {
      Some(path) => Ok(cx.boxed(RefCell::new(Path2D::from(rule_independent(path))))),
      None => cx.throw_error("path operation failed")
    }
  }else{
    cx.throw_error("pathOp must be Difference, Intersect, Union, XOR, or Complement")
  }
}

pub fn interpolate(mut cx: FunctionContext) -> JsResult<BoxedPath2D> {
  let this = cx.argument::<BoxedPath2D>(0)?;
  let other = path2d_arg(&mut cx, 1)?;
  let weight = float_arg(&mut cx, 2, "weight")?;

  let this = this.borrow();
  let other = other.borrow();
  // reverse path order since 0..1 = this..other is a less non-sensical mapping than the default
  if let Some(path) = other.path().interpolate(&this.path(), weight){
    Ok(cx.boxed(RefCell::new(Path2D::from(path))))
  }else{
    cx.throw_type_error("Can only interpolate between two Path2D objects with the same number of points and control points")
  }
}

// Returns a path with only non-overlapping contours that describe the same area as the original path
pub fn simplify(mut cx: FunctionContext) -> JsResult<BoxedPath2D> {
  let this = cx.argument::<BoxedPath2D>(0)?;
  let rule = fill_rule_arg_or(&mut cx, 1, "nonzero")?;
  let this = this.borrow();

  let mut path = this.path();
  path.set_fill_type(rule);

  let new_path = Path2D::from(rule_independent(match this.path().set_fill_type(rule).simplify(){
    Some(simpler) => simpler,
    None => path
  }));

  Ok(cx.boxed(RefCell::new(new_path)))
}

// Returns a path that can be drawn with a nonzero fill but looks like the original drawn with evenodd
pub fn unwind(mut cx: FunctionContext) -> JsResult<BoxedPath2D> {
  let this = cx.argument::<BoxedPath2D>(0)?;
  let mut this = this.borrow_mut();

  this.update().set_fill_type(PathFillType::EvenOdd);

  let path = this.path();
  let new_path = Path2D::from(match path.as_winding(){
    Some(rewound) => rewound,
    None => path
  });

  Ok(cx.boxed(RefCell::new(new_path)))
}

// Returns a copy whose points have been shifted by (dx, dy)
pub fn offset(mut cx: FunctionContext) -> JsResult<BoxedPath2D> {
  let this = cx.argument::<BoxedPath2D>(0)?;
  let dx = float_arg(&mut cx, 1, "dx")?;
  let dy = float_arg(&mut cx, 2, "dy")?;

  let this = this.borrow();
  let path = this.snapshot_transformed(&Matrix::translate((dx, dy)));
  Ok(cx.boxed(RefCell::new(Path2D::from(path))))
}

// Returns a copy whose points have been transformed by a given matrix
pub fn transform(mut cx: FunctionContext) -> JsResult<BoxedPath2D> {
  let this = cx.argument::<BoxedPath2D>(0)?;
  let matrix = matrix_arg(&mut cx, 1)?;

  let this = this.borrow();
  let path = this.snapshot_transformed(&matrix);
  Ok(cx.boxed(RefCell::new(Path2D::from(path))))
}

// Returns a copy where every sharp junction to an arcTo-style rounded corner
pub fn round(mut cx: FunctionContext) -> JsResult<BoxedPath2D> {
  let this = cx.argument::<BoxedPath2D>(0)?;
  let radius = float_arg(&mut cx, 1, "radius")?;

  let this = this.borrow();
  let path = this.path();
  let bounds = *path.bounds();
  let stroke_rec = StrokeRec::new_hairline();

  if let Some(rounder) = PathEffect::corner_path(radius){
    if let Some((rounded, _)) = rounder.filter_path(&path, &stroke_rec, bounds){
      return Ok(cx.boxed(RefCell::new(Path2D::from(rounded))))
    }
  }

  Ok(cx.boxed(RefCell::new(Path2D::from(path))))
}

// Clips a proportional segment out of the middle of the path (or the edges if invert=true)
pub fn trim(mut cx: FunctionContext) -> JsResult<BoxedPath2D> {
  let this = cx.argument::<BoxedPath2D>(0)?;
  let begin = float_arg_or_bail(&mut cx, 1, "begin")?;
  let end = float_arg_or_bail(&mut cx, 2, "end")?;
  let invert = bool_arg_or(&mut cx, 3, false);

  let this = this.borrow();
  let path = this.path();
  let bounds = *path.bounds();
  let stroke_rec = StrokeRec::new_hairline();
  let mode = if invert{ trim_path_effect::Mode::Inverted }else{ trim_path_effect::Mode::Normal };

  if let Some(trimmer) = PathEffect::trim(begin, end, mode){
    if let Some((trimmed, _)) = trimmer.filter_path(&path, &stroke_rec, bounds){
      return Ok(cx.boxed(RefCell::new(Path2D::from(trimmed))))
    }
  }

  Ok(cx.boxed(RefCell::new(Path2D::from(path))))
}

// Discretizes the path at a fixed segment length then randomly offsets the points
pub fn jitter(mut cx: FunctionContext) -> JsResult<BoxedPath2D> {
  let this = cx.argument::<BoxedPath2D>(0)?;
  let seg_len = float_arg_or_bail(&mut cx, 1, "segmentLength")?;
  let std_dev = float_arg_or_bail(&mut cx, 2, "variance")?;
  let seed = float_arg_or(&mut cx, 3, 0.0) as u32;

  let this = this.borrow();
  let path = this.path();
  let bounds = *path.bounds();
  let stroke_rec = StrokeRec::new_hairline();

  if let Some(trimmer) = PathEffect::discrete(seg_len, std_dev, Some(seed)){
    if let Some((jittered, _)) = trimmer.filter_path(&path, &stroke_rec, bounds){
      return Ok(cx.boxed(RefCell::new(Path2D::from(jittered))))
    }
  }

  Ok(cx.boxed(RefCell::new(Path2D::from(path))))
}

// Returns the computed `tight` bounds that contain all the points, control points, and connecting contours
pub fn bounds(mut cx: FunctionContext) -> JsResult<JsObject> {
  let this = cx.argument::<BoxedPath2D>(0)?;
  let this = this.borrow();

  let b = this.path().compute_tight_bounds();

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

  Ok(cx.boolean(this.path().contains((x,y))))
}

// total arc length of all the path's contours
pub fn get_length(mut cx: FunctionContext) -> JsResult<JsNumber> {
  let this = cx.argument::<BoxedPath2D>(0)?;
  let this = this.borrow();

  Ok(cx.number(this.measure().total))
}

// the point a given distance along the path (or null if the path has no length)
pub fn position_at(mut cx: FunctionContext) -> JsResult<JsValue> {
  let this = cx.argument::<BoxedPath2D>(0)?;
  let distance = distance_arg(&mut cx, 1);
  let this = this.borrow();

  match this.measure().locate(distance){
    Some((Point{x, y}, _)) => {
      let js_object = cx.empty_object();
      let x = cx.number(x);
      let y = cx.number(y);
      js_object.set(&mut cx, "x", x)?;
      js_object.set(&mut cx, "y", y)?;
      Ok(js_object.upcast())
    }
    None => Ok(cx.null().upcast())
  }
}

// tangent direction (in radians) at given distance along the path (or null if the path has no length)
pub fn tangent_at(mut cx: FunctionContext) -> JsResult<JsValue> {
  let this = cx.argument::<BoxedPath2D>(0)?;
  let distance = distance_arg(&mut cx, 1);
  let this = this.borrow();

  match this.measure().heading(distance){
    Some(heading) => Ok(cx.number(heading).upcast()),
    None => Ok(cx.null().upcast())
  }
}

// convenience method returning tangent-π/2, normalized to lie in the (-π, π] range
pub fn normal_at(mut cx: FunctionContext) -> JsResult<JsValue> {
  let this = cx.argument::<BoxedPath2D>(0)?;
  let distance = distance_arg(&mut cx, 1);
  let this = this.borrow();

  match this.measure().heading(distance){
    Some(heading) => {
      let normal = heading - FRAC_PI_2;
      let normal = if normal <= -PI { normal + TAU } else { normal };
      Ok(cx.number(normal).upcast())
    }
    None => Ok(cx.null().upcast())
  }
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

  let path = this.path();
  let mut weights = path::Iter::new(&path, false);
  let iter = path::Iter::new(&path, false);

  let mut edges = vec![];
  for (verb, points) in iter{
    weights.next();

    if let Some(edge) = from_verb(verb){
      let cmd = cx.string(edge);
      let segment = JsArray::new(&mut cx, 1 + points.len());
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

  let verbs = JsArray::new(&mut cx, edges.len());
  for (i, segment) in edges.iter().enumerate(){
    verbs.set(&mut cx, i as u32, *segment)?;
  }

  Ok(verbs)
}

pub fn get_d(mut cx: FunctionContext) -> JsResult<JsString> {
  let this = cx.argument::<BoxedPath2D>(0)?;
  let this = this.borrow();
  Ok(cx.string(this.path().to_svg()))
}

pub fn set_d(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedPath2D>(0)?;
  let svg_string = string_arg(&mut cx, 1, "svgPath")?;
  let mut this = this.borrow_mut();

  if let Some(path) = Path::from_svg(svg_string){
    this.update().reset().add_path(&path, None);
    Ok(cx.undefined())
  }else{
    cx.throw_type_error("Expected a valid SVG path string")
  }
}
