#![allow(non_snake_case)]
use std::cell::{RefCell};
use std::rc::Rc;
use neon::prelude::*;
use skia_safe::{Shader, Color, Point, TileMode, Matrix};
use skia_safe::{gradient_shader, gradient_shader::GradientShaderColors::Colors};

use crate::utils::*;

enum Gradient{
  Linear{
    start:Point,
    end:Point,
    stops:Vec<f32>,
    colors:Vec<Color>,
  },
  Radial{
    start_point:Point,
    start_radius:f32,
    end_point:Point,
    end_radius:f32,
    stops:Vec<f32>,
    colors:Vec<Color>,
  },
  Conic{
    center:Point,
    angle:f32,
    stops:Vec<f32>,
    colors:Vec<Color>,
  }
}

impl Gradient{
  fn get_stops(&self) -> &Vec<f32>{
    match self{
      Gradient::Linear{stops, ..} => stops,
      Gradient::Radial{stops, ..} => stops,
      Gradient::Conic{stops, ..} => stops,
    }
  }

  fn get_colors(&self) -> &Vec<Color>{
    match self{
      Gradient::Linear{colors, ..} => colors,
      Gradient::Radial{colors, ..} => colors,
      Gradient::Conic{colors, ..} => colors,
    }
  }

  fn add_stop(&mut self, offset: f32, color:Color){
    let stops = self.get_stops();

    // insert the new entries at the right index to keep the vectors sorted
    let idx = stops.binary_search_by(|n| (n-f32::EPSILON).partial_cmp(&offset).unwrap()).unwrap_or_else(|x| x);
    match self{
      Gradient::Linear{colors, stops, ..} => { colors.insert(idx, color); stops.insert(idx, offset); },
      Gradient::Radial{colors, stops, ..} => { colors.insert(idx, color); stops.insert(idx, offset); },
      Gradient::Conic{colors, stops, ..} => { colors.insert(idx, color); stops.insert(idx, offset); },
    };
  }
}

pub type BoxedCanvasGradient = JsBox<RefCell<CanvasGradient>>;
impl Finalize for CanvasGradient {}

#[derive(Clone)]
pub struct CanvasGradient{
  gradient:Rc<RefCell<Gradient>>
}

impl CanvasGradient{
  pub fn shader(&self) -> Option<Shader>{
    match &*self.gradient.borrow(){
      Gradient::Linear{start, end, stops, colors} => {
        gradient_shader::linear((*start, *end), Colors(colors), Some(stops.as_slice()), TileMode::Clamp, None, None)
      },
      Gradient::Radial{start_point, start_radius, end_point, end_radius, stops, colors} => {
        gradient_shader::two_point_conical(
          *start_point, *start_radius,
          *end_point, *end_radius,
          Colors(colors), Some(stops.as_slice()),
          TileMode::Clamp, None, None)
      },
      Gradient::Conic{center, angle, stops, colors} => {
        let Point{x, y} = *center;
        let mut rotated = Matrix::new_identity();
        rotated
          .pre_translate((x, y))
          .pre_rotate(*angle, None)
          .pre_translate((-x, -y));

        gradient_shader::sweep(
          *center,
          Colors(colors),
          Some(stops.as_slice()),
          TileMode::Clamp,
          None, // angles
          None, // flags
          Some(&rotated), // local_matrix

        )
      }
    }
  }

  pub fn add_color_stop(&mut self, offset: f32, color:Color){
    self.gradient.borrow_mut().add_stop(offset, color);
  }

  pub fn is_opaque(&self) -> bool{
    // true if all colors are 100% opaque
    let gradient = self.gradient.borrow();
    !gradient.get_colors().iter().any(|c| c.a() < 255)
  }
}

//
// -- Javascript Methods --------------------------------------------------------------------------
//

pub fn linear(mut cx: FunctionContext) -> JsResult<BoxedCanvasGradient> {
  let nums = &float_args(&mut cx, &["x1", "y1", "x2", "y2"])?[..4];
  let [x1, y1, x2, y2] = nums else{ panic!() };

  let start = Point::new(*x1, *y1);
  let end = Point::new(*x2, *y2);
  let ramp = Gradient::Linear{ start, end, stops:vec![], colors:vec![] };
  let canvas_gradient = CanvasGradient{ gradient:Rc::new(RefCell::new(ramp)) };
  let this = RefCell::new(canvas_gradient);
  Ok(cx.boxed(this))
}

pub fn radial(mut cx: FunctionContext) -> JsResult<BoxedCanvasGradient> {
  let nums = &float_args(&mut cx, &["x1", "y1", "r1", "x2", "y2", "r2"])?[..6];
  let [x1, y1, r1, x2, y2, r2] = nums else{ panic!() };

  let start_point = Point::new(*x1, *y1);
  let end_point = Point::new(*x2, *y2);
  let bloom = Gradient::Radial{ start_point, start_radius:*r1, end_point, end_radius:*r2, stops:vec![], colors:vec![] };
  let canvas_gradient = CanvasGradient{ gradient:Rc::new(RefCell::new(bloom)) };
  let this = RefCell::new(canvas_gradient);
  Ok(cx.boxed(this))
}

pub fn conic(mut cx: FunctionContext) -> JsResult<BoxedCanvasGradient> {
  let nums = &float_args(&mut cx, &["theta", "x", "y"])?[..3];
  let [theta, x, y] = nums else{ panic!() };

  let center = Point::new(*x, *y);
  let angle = to_degrees(*theta);
  let sweep = Gradient::Conic{ center, angle, stops:vec![], colors:vec![] };
  let canvas_gradient = CanvasGradient{ gradient:Rc::new(RefCell::new(sweep)) };
  let this = RefCell::new(canvas_gradient);
  Ok(cx.boxed(this))
}

pub fn addColorStop(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedCanvasGradient>(0)?;
  let mut this = this.borrow_mut();

  let offset = float_arg(&mut cx, 1, "offset")?;
  if offset < 0.0 || offset > 1.0{
    return cx.throw_range_error("Color stop offsets must be between 0.0 and 1.0");
  }

  if let Some(color) = opt_color_arg(&mut cx, 2) {
    this.add_color_stop(offset, color);
  }else{
    return cx.throw_type_error(match cx.len(){
      3 => "Could not be parsed as a color",
      _ => "not enough arguments"
    })
  }

  Ok(cx.undefined())
}

pub fn repr(mut cx: FunctionContext) -> JsResult<JsString> {
  let this = cx.argument::<BoxedCanvasGradient>(0)?;
  let this = this.borrow();
  let gradient = Rc::clone(&this.gradient);

  let style = match &*gradient.borrow(){
    Gradient::Linear{..} => "Linear",
    Gradient::Radial{..} => "Radial",
    Gradient::Conic{..} => "Conic",
  };

  Ok(cx.string(style))
}