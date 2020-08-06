use std::f32::consts::PI;
use neon::prelude::*;
use skia_safe::{Path, Point, PathDirection, Rect, Matrix};
use skia_safe::path::{AddPathMode};

use crate::utils::*;

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
    let tao = 2.0 * PI;
    let mut new_start_angle = start_angle % tao;
    if new_start_angle < 0.0 {
      new_start_angle += tao;
    }
    let delta = new_start_angle - start_angle;
    let start_angle = new_start_angle;
    let mut end_angle = end_angle + delta;

    // Based off of AdjustEndAngle in Chrome.
    if !ccw && (end_angle - start_angle) >= tao {
      end_angle = start_angle + tao; // Draw complete ellipse
    } else if ccw && (start_angle - end_angle) >= tao {
      end_angle = start_angle - tao; // Draw complete ellipse
    } else if !ccw && start_angle > end_angle {
      end_angle = start_angle + (tao - (start_angle - end_angle) % tao);
    } else if ccw && start_angle < end_angle {
      end_angle = start_angle - (tao - (end_angle - start_angle) % tao);
    }

    // Based off of Chrome's implementation in
    // https://cs.chromium.org/chromium/src/third_party/blink/renderer/platform/graphics/path.cc
    // of note, can't use addArc or addOval because they close the arc, which
    // the spec says not to do (unless the user explicitly calls closePath).
    // This throws off points being in/out of the arc.
    let oval = Rect::new(x - x_radius, y - y_radius, x + x_radius, y + y_radius);
    let mut rotated = Matrix::new_identity();
    rotated.pre_rotate(to_degrees(rotation), None);
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

declare_types! {
  pub class JsPath2D for Path2D {
    init(_) {
      Ok(Path2D{ path:Path::new() })
    }

    constructor(mut cx){
      let mut this = cx.this();

      if cx.len() > 0 {
        let arg = cx.argument::<JsValue>(0)?;

        if arg.is_a::<JsPath2D>(){
          let that = arg.downcast::<JsPath2D>().or_throw(&mut cx)?;
          cx.borrow(&that, |that| {
            cx.borrow_mut(&mut this, |mut this| this.path = that.path.clone())
          });
        }else if arg.is_a::<JsString>(){
          let svg_string = string_arg(&mut cx, 0, "svgPath")?;
          if let Some(svg_path) = Path::from_svg(svg_string){
            cx.borrow_mut(&mut this, |mut this| this.path = svg_path);
          }
        }
      }

      Ok(None)
    }

    // Adds a path to the current path.
    method addPath(mut cx){
      let mut this = cx.this();
      let arg = cx.argument::<JsValue>(0)?;
      if !arg.is_a::<JsPath2D>(){
        return cx.throw_type_error("Argument 1 ('path') to Path2D.addPath must be an instance of Path2D")
      }
      let matrix = match cx.argument_opt(1){
        Some(val) => {
          let arg = val.downcast::<JsArray>().or_throw(&mut cx)?.to_vec(&mut cx)?;
          matrix_in(&mut cx, &arg)?
        },
        None => Matrix::new_identity()
      };

      let that = arg.downcast::<JsPath2D>().or_throw(&mut cx)?;
      cx.borrow(&that, |that| {
        cx.borrow_mut(&mut this, |mut this|{
          this.path.add_path_matrix(&that.path, &matrix, AddPathMode::Append);
        })
      });
      Ok(cx.undefined().upcast())
    }


    // Causes the point of the pen to move back to the start of the current sub-path. It tries to draw a straight line from the current point to the start. If the shape has already been closed or has only one point, this function does nothing.
    method closePath(mut cx){
      let mut this = cx.this();
      cx.borrow_mut(&mut this, |mut this| {
        this.path.close();
      });

      Ok(cx.undefined().upcast())
    }


    // Moves the starting point of a new sub-path to the (x, y) coordinates.
    method moveTo(mut cx){
      let mut this = cx.this();
      let x = float_arg(&mut cx, 0, "x")?;
      let y = float_arg(&mut cx, 1, "y")?;
      cx.borrow_mut(&mut this, |mut this| {
        this.path.move_to((x, y));
      });

      Ok(cx.undefined().upcast())
    }


    // Connects the last point in the subpath to the (x, y) coordinates with a straight line.
    method lineTo(mut cx){
      let mut this = cx.this();
      let x = float_arg(&mut cx, 0, "x")?;
      let y = float_arg(&mut cx, 1, "y")?;
      cx.borrow_mut(&mut this, |mut this| {
        this.scoot(x, y);
        this.path.line_to((x, y));
      });

      Ok(cx.undefined().upcast())
    }


    // Adds a cubic Bézier curve to the path. It requires three points. The first two points are control points and the third one is the end point. The starting point is the last point in the current path, which can be changed using moveTo() before creating the Bézier curve.
    method bezierCurveTo(mut cx){
      let mut this = cx.this();
      let nums = float_args(&mut cx, 0..6)?;
      if let [cp1x, cp1y, cp2x, cp2y, x, y] = nums.as_slice(){
        cx.borrow_mut(&mut this, |mut this| {
          this.scoot(*cp1x, *cp1y);
          this.path.cubic_to((*cp1x, *cp1y), (*cp2x, *cp2y), (*x, *y));
        });
      }

      Ok(cx.undefined().upcast())
    }


    // Adds a quadratic Bézier curve to the current path.
    method quadraticCurveTo(mut cx){
      let mut this = cx.this();
      let nums = float_args(&mut cx, 0..4)?;
      if let [cpx, cpy, x, y] = nums.as_slice(){
        cx.borrow_mut(&mut this, |mut this| {
          this.scoot(*cpx, *cpy);
          this.path.quad_to((*cpx, *cpy), (*x, *y));
        });
      }

      Ok(cx.undefined().upcast())
    }


    // Adds an arc to the path which is centered at (x, y) position with radius r starting at startAngle and ending at endAngle going in the given direction by anticlockwise (defaulting to clockwise).
    method arc(mut cx){
      let mut this = cx.this();
      let nums = float_args(&mut cx, 0..5)?;
      let ccw = bool_arg_or(&mut cx, 5, false);

      if let [x, y, radius, start_angle, end_angle] = nums.as_slice(){
        cx.borrow_mut(&mut this, |mut this| {
          this.add_ellipse((*x, *y), (*radius, *radius), 0.0, *start_angle, *end_angle, ccw);
        });
      }

      Ok(cx.undefined().upcast())
    }


    // Adds a circular arc to the path with the given control points and radius, connected to the previous point by a straight line.
    method arcTo(mut cx){
      let mut this = cx.this();
      let coords = float_args(&mut cx, 0..4)?;
      let radius = float_arg(&mut cx, 4, "radius")?;

      if let [x1, y1, x2, y2] = coords.as_slice(){
        cx.borrow_mut(&mut this, |mut this| {
          this.scoot(*x1, *y1);
          this.path.arc_to_tangent((*x1, *y1), (*x2, *y2), radius);
        });
      }

      Ok(cx.undefined().upcast())
    }


    // Adds an elliptical arc to the path which is centered at (x, y) position with the radii radiusX and radiusY starting at startAngle and ending at endAngle going in the given direction by anticlockwise (defaulting to clockwise).
    method ellipse(mut cx){
      let mut this = cx.this();
      let nums = float_args(&mut cx, 0..7)?;
      let ccw = bool_arg(&mut cx, 7, "isCCW")?;

      if let [x, y, x_radius, y_radius, rotation, start_angle, end_angle] = nums.as_slice(){
        if *x_radius < 0.0 || *y_radius < 0.0 {
          return cx.throw_error("radii cannot be negative")
        }

        cx.borrow_mut(&mut this, |mut this| {
          this.add_ellipse((*x, *y), (*x_radius, *y_radius), *rotation, *start_angle, *end_angle, ccw);
        });
      }

      Ok(cx.undefined().upcast())
    }

    // Creates a path for a rectangle at position (x, y) with a size that is determined by width and height.
    method rect(mut cx){
      let mut this = cx.this();
      let nums = float_args(&mut cx, 0..4)?;
      if let [x, y, w, h] = nums.as_slice(){
        let rect = Rect::from_xywh(*x, *y, *w, *h);
        cx.borrow_mut(&mut this, |mut this| {
          this.path.add_rect(rect, Some((PathDirection::CW, 0)));
        });
      }

      Ok(cx.undefined().upcast())
    }

  }
}