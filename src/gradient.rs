#![allow(non_snake_case)]
use std::cell::{RefCell};
use std::rc::Rc;
use neon::prelude::*;
use skia_safe::{Shader, Color4f, Point, TileMode, Matrix};
use skia_safe::gradient::{self, Colors as GradientColors, Interpolation, interpolation::{ColorSpace as InterpColorSpace, HueMethod, InPremul}};

use crate::utils::*;

enum Gradient{
  Linear{
    start:Point,
    end:Point,
    stops:Vec<f32>,
    colors:Vec<Color4f>,
    interp:Interpolation,
  },
  Radial{
    start_point:Point,
    start_radius:f32,
    end_point:Point,
    end_radius:f32,
    stops:Vec<f32>,
    colors:Vec<Color4f>,
    interp:Interpolation,
  },
  Conic{
    center:Point,
    angle:f32,
    stops:Vec<f32>,
    colors:Vec<Color4f>,
    interp:Interpolation,
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

  fn get_colors(&self) -> &Vec<Color4f>{
    match self{
      Gradient::Linear{colors, ..} => colors,
      Gradient::Radial{colors, ..} => colors,
      Gradient::Conic{colors, ..} => colors,
    }
  }

  fn get_interpolation(&self) -> Interpolation{
    match self{
      Gradient::Linear{interp, ..} => *interp,
      Gradient::Radial{interp, ..} => *interp,
      Gradient::Conic{interp, ..} => *interp,
    }
  }

  fn set_interpolation(&mut self, next:Interpolation){
    match self{
      Gradient::Linear{interp, ..} => *interp = next,
      Gradient::Radial{interp, ..} => *interp = next,
      Gradient::Conic{interp, ..} => *interp = next,
    }
  }

  fn add_stop(&mut self, offset: f32, color:Color4f){
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
    let gradient = self.gradient.borrow();
    let colors = gradient.get_colors();
    let stops = gradient.get_stops();
    let spec = gradient::Gradient::new(
      GradientColors::new(colors, Some(stops.as_slice()), TileMode::Clamp, None),
      gradient.get_interpolation(), // defaults to sRGB, no-premul
    );

    match &*gradient{
      Gradient::Linear{start, end, ..} => {
        gradient::shaders::linear_gradient((*start, *end), &spec, None)
      },
      Gradient::Radial{start_point, start_radius, end_point, end_radius, ..} => {
        gradient::shaders::two_point_conical_gradient(
          (*start_point, *start_radius),
          (*end_point, *end_radius),
          &spec, None)
      },
      Gradient::Conic{center, angle, ..} => {
        let Point{x, y} = *center;
        let mut rotated = Matrix::new_identity();
        rotated
          .pre_translate((x, y))
          .pre_rotate(*angle, None)
          .pre_translate((-x, -y));

        gradient::shaders::sweep_gradient(
          *center,
          (0.0, 360.0), // same angular range the old api defaulted to
          &spec,
          Some(&rotated), // local_matrix
        )
      }
    }
  }

  pub fn add_color_stop(&mut self, offset: f32, color:Color4f){
    self.gradient.borrow_mut().add_stop(offset, color);
  }

  fn interp(&self) -> Interpolation{
    self.gradient.borrow().get_interpolation()
  }
  fn set_interp(&mut self, interp:Interpolation){
    self.gradient.borrow_mut().set_interpolation(interp);
  }

  pub fn color_interpolation_method(&self) -> &'static str{
    color_interpolation_to_str(self.interp().color_space)
  }
  pub fn set_color_interpolation_method(&mut self, space:InterpColorSpace){
    let mut interp = self.interp(); interp.color_space = space; self.set_interp(interp);
  }

  pub fn hue_interpolation_method(&self) -> &'static str{
    hue_method_to_str(self.interp().hue_method)
  }
  pub fn set_hue_interpolation_method(&mut self, method:HueMethod){
    let mut interp = self.interp(); interp.hue_method = method; self.set_interp(interp);
  }

  pub fn premultiplied_alpha(&self) -> bool{
    matches!(self.interp().in_premul, InPremul::Yes)
  }
  pub fn set_premultiplied_alpha(&mut self, premul:bool){
    let mut interp = self.interp();
    interp.in_premul = if premul { InPremul::Yes } else { InPremul::No };
    self.set_interp(interp);
  }

  pub fn is_opaque(&self) -> bool{
    // true if all colors are 100% opaque
    let gradient = self.gradient.borrow();
    !gradient.get_colors().iter().any(|c| c.a < 1.0)
  }
}

//
// -- Javascript Methods --------------------------------------------------------------------------
//

pub fn linear(mut cx: FunctionContext) -> JsResult<BoxedCanvasGradient> {
  let nums = &float_args(&mut cx, &["x1", "y1", "x2", "y2"])?[..4];
  let [x1, y1, x2, y2] = nums else{ panic!() };
  let interp = default_interpolation();

  let start = Point::new(*x1, *y1);
  let end = Point::new(*x2, *y2);
  let ramp = Gradient::Linear{ start, end, stops:vec![], colors:vec![], interp };
  let canvas_gradient = CanvasGradient{ gradient:Rc::new(RefCell::new(ramp)) };
  let this = RefCell::new(canvas_gradient);
  Ok(cx.boxed(this))
}

pub fn radial(mut cx: FunctionContext) -> JsResult<BoxedCanvasGradient> {
  let nums = &float_args(&mut cx, &["x1", "y1", "r1", "x2", "y2", "r2"])?[..6];
  let [x1, y1, r1, x2, y2, r2] = nums else{ panic!() };
  let interp = default_interpolation();

  let start_point = Point::new(*x1, *y1);
  let end_point = Point::new(*x2, *y2);
  let bloom = Gradient::Radial{ start_point, start_radius:*r1, end_point, end_radius:*r2, stops:vec![], colors:vec![], interp };
  let canvas_gradient = CanvasGradient{ gradient:Rc::new(RefCell::new(bloom)) };
  let this = RefCell::new(canvas_gradient);
  Ok(cx.boxed(this))
}

pub fn conic(mut cx: FunctionContext) -> JsResult<BoxedCanvasGradient> {
  let nums = &float_args(&mut cx, &["theta", "x", "y"])?[..3];
  let [theta, x, y] = nums else{ panic!() };
  let interp = default_interpolation();

  let center = Point::new(*x, *y);
  let angle = to_degrees(*theta);
  let sweep = Gradient::Conic{ center, angle, stops:vec![], colors:vec![], interp };
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

  if let Some(color) = opt_color_arg_4f(&mut cx, 2) {
    this.add_color_stop(offset, color);
  }else{
    return cx.throw_type_error(match cx.len(){
      3 => "Could not be parsed as a color",
      _ => "not enough arguments"
    })
  }

  Ok(cx.undefined())
}

pub fn get_colorInterpolationMethod(mut cx: FunctionContext) -> JsResult<JsString> {
  let this = cx.argument::<BoxedCanvasGradient>(0)?;
  let name = this.borrow().color_interpolation_method();
  Ok(cx.string(name))
}

pub fn set_colorInterpolationMethod(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedCanvasGradient>(0)?;
  let name = string_arg(&mut cx, 1, "colorInterpolationMethod")?;
  match color_interpolation_from_str(&name){
    Some(space) => { this.borrow_mut().set_color_interpolation_method(space); Ok(cx.undefined()) },
    None => cx.throw_type_error(format!("Unsupported colorInterpolationMethod \"{}\"", name)),
  }
}

pub fn get_hueInterpolationMethod(mut cx: FunctionContext) -> JsResult<JsString> {
  let this = cx.argument::<BoxedCanvasGradient>(0)?;
  let name = this.borrow().hue_interpolation_method();
  Ok(cx.string(name))
}

pub fn set_hueInterpolationMethod(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedCanvasGradient>(0)?;
  let name = string_arg(&mut cx, 1, "hueInterpolationMethod")?;
  match hue_method_from_str(&name){
    Some(method) => { this.borrow_mut().set_hue_interpolation_method(method); Ok(cx.undefined()) },
    None => cx.throw_type_error(format!("Unsupported hueInterpolationMethod \"{}\"", name)),
  }
}

pub fn get_premultipliedAlpha(mut cx: FunctionContext) -> JsResult<JsBoolean> {
  let this = cx.argument::<BoxedCanvasGradient>(0)?;
  let premul = this.borrow().premultiplied_alpha();
  Ok(cx.boolean(premul))
}

pub fn set_premultipliedAlpha(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedCanvasGradient>(0)?;
  let premul = bool_arg(&mut cx, 1, "premultipliedAlpha")?;
  this.borrow_mut().set_premultiplied_alpha(premul);
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
