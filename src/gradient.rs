#![allow(dead_code)]
#![allow(non_snake_case)]
use std::cell::RefCell;
use std::sync::{Arc, Mutex};
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

pub type BoxedCanvasGradient = JsBox<RefCell<CanvasGradient>>;
impl Finalize for CanvasGradient {}

#[derive(Clone)]
pub struct CanvasGradient{
  gradient:Arc<Mutex<Gradient>>
}

impl CanvasGradient{
  pub fn shader(&self) -> Option<Shader>{

    let gradient = Arc::clone(&self.gradient);
    let gradient = gradient.lock().unwrap();

    match &*gradient{
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
    let gradient = Arc::clone(&self.gradient);
    let mut gradient = gradient.lock().unwrap();

    let stops = match &*gradient{
      Gradient::Linear{stops, ..} => stops,
      Gradient::Radial{stops, ..} => stops,
      Gradient::Conic{stops, ..} => stops,
    };

    // insert the new entries at the right index to keep the vectors sorted
    let idx = stops.binary_search_by(|n| (n-f32::EPSILON).partial_cmp(&offset).unwrap()).unwrap_or_else(|x| x);
    match &mut *gradient{
      Gradient::Linear{colors, stops, ..} => { colors.insert(idx, color); stops.insert(idx, offset); },
      Gradient::Radial{colors, stops, ..} => { colors.insert(idx, color); stops.insert(idx, offset); },
      Gradient::Conic{colors, stops, ..} => { colors.insert(idx, color); stops.insert(idx, offset); },
    };
  }

  pub fn is_opaque(&self) -> bool{
    let gradient = Arc::clone(&self.gradient);
    let gradient = gradient.lock().unwrap();

    let colors = match &*gradient{
      Gradient::Linear{colors, ..} => colors,
      Gradient::Radial{colors, ..} => colors,
      Gradient::Conic{colors, ..} => colors,
    };

    // true if all colors are 100% opaque
    !colors.iter().any(|c| c.a() < 255)
  }
}

//
// -- Javascript Methods --------------------------------------------------------------------------
//

pub fn linear(mut cx: FunctionContext) -> JsResult<BoxedCanvasGradient> {
  if let [x1, y1, x2, y2] = opt_float_args(&mut cx, 1..5).as_slice(){
    let start = Point::new(*x1, *y1);
    let end = Point::new(*x2, *y2);
    let ramp = Gradient::Linear{ start, end, stops:vec![], colors:vec![] };
    let canvas_gradient = CanvasGradient{ gradient:Arc::new(Mutex::new(ramp)) };
    let this = RefCell::new(canvas_gradient);
    Ok(cx.boxed(this))
  }else{
    cx.throw_type_error(match cx.len(){
      5.. => "The provided value is non-finite",
      _ => "not enough arguments"
    })
  }
}

pub fn radial(mut cx: FunctionContext) -> JsResult<BoxedCanvasGradient> {
  if let [x1, y1, r1, x2, y2, r2] = opt_float_args(&mut cx, 1..7).as_slice(){
    let start_point = Point::new(*x1, *y1);
    let end_point = Point::new(*x2, *y2);
    let bloom = Gradient::Radial{ start_point, start_radius:*r1, end_point, end_radius:*r2, stops:vec![], colors:vec![] };
    let canvas_gradient = CanvasGradient{ gradient:Arc::new(Mutex::new(bloom)) };
    let this = RefCell::new(canvas_gradient);
    Ok(cx.boxed(this))
  }else{
    cx.throw_type_error(match cx.len(){
      7.. => "The provided value is non-finite",
      _ => "not enough arguments"
    })

  }
}

pub fn conic(mut cx: FunctionContext) -> JsResult<BoxedCanvasGradient> {
  if let [theta, x, y] = opt_float_args(&mut cx, 1..4).as_slice(){
    let center = Point::new(*x, *y);
    let angle = to_degrees(*theta);
    let sweep = Gradient::Conic{ center, angle, stops:vec![], colors:vec![] };
    let canvas_gradient = CanvasGradient{ gradient:Arc::new(Mutex::new(sweep)) };
    let this = RefCell::new(canvas_gradient);
    Ok(cx.boxed(this))
  }else{
    cx.throw_type_error(match cx.len(){
      4.. => "The provided value is non-finite",
      _ => "not enough arguments"
    })
  }
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
  let gradient = Arc::clone(&this.gradient);
  let gradient = gradient.lock().unwrap();

  let style = match &*gradient{
    Gradient::Linear{..} => "Linear",
    Gradient::Radial{..} => "Radial",
    Gradient::Conic{..} => "Conic",
  };

  Ok(cx.string(style))
}