use neon::prelude::*;
use skia_safe::{Shader, Color, Point, TileMode, gradient_shader, gradient_shader::GradientShaderColors::Colors};

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
  }
}

pub struct CanvasGradient{
  gradient:Option<Gradient>
}

impl CanvasGradient{
    #![allow(dead_code)]
    pub fn shader(&self) -> Option<Shader>{
    match self.gradient.as_ref(){
      Some(gradient) => match gradient{
        Gradient::Linear{start, end, stops, colors} => {
          gradient_shader::linear((*start, *end), Colors(&colors), Some(stops.as_ref()), TileMode::Clamp, None, None)
        },
        Gradient::Radial{start_point, start_radius, end_point, end_radius, stops, colors} => {
          gradient_shader::two_point_conical(
            *start_point, *start_radius,
            *end_point, *end_radius,
            Colors(&colors), Some(stops.as_ref()),
            TileMode::Clamp, None, None)
        },
      },
      None => None
    }
  }
}

declare_types! {
  pub class JsCanvasGradient for CanvasGradient {
    init(_) {
      Ok(CanvasGradient{ gradient:None })
    }

    constructor(mut cx){
      let mut this = cx.this();
      let kind = string_arg(&mut cx, 0, "gradientType")?;
      let gradient:Option<Gradient> = match kind.to_lowercase().as_str(){
        "linear" => {
          if let [x1, y1, x2, y2] = float_args(&mut cx, 1..5)?.as_slice(){
            let start = Point::new(*x1, *y1);
            let end = Point::new(*x2, *y2);
            Some(Gradient::Linear{ start, end, stops:vec![], colors:vec![]})
          }else{
            return cx.throw_type_error("Not enough arguments")
          }
        },
        "radial" => {
          if let [x1, y1, r1, x2, y2, r2] = float_args(&mut cx, 1..7)?.as_slice(){
            let start_point = Point::new(*x1, *y1);
            let end_point = Point::new(*x2, *y2);
            Some(Gradient::Radial{ start_point, start_radius:*r1, end_point, end_radius:*r2, stops:vec![], colors:vec![]})
          }else{
            return cx.throw_type_error("Not enough arguments")
          }
        },
        _ => None
      };

      cx.borrow_mut(&mut this, |mut this| { this.gradient = gradient });

      Ok(None)
    }

    method add_color_stop(mut cx){
      let mut this = cx.this();
      let offset = float_arg(&mut cx, 0, "offset")?;
      let color = color_args(&mut cx, 1..5, "color")?;
      cx.borrow_mut(&mut this, |mut this| {
        if let Some(gradient) = this.gradient.as_mut(){
          match gradient{
            Gradient::Linear{colors, stops, ..} => { colors.push(color); stops.push(offset); },
            Gradient::Radial{colors, stops, ..} => { colors.push(color); stops.push(offset); },
          };
        }
      });
      Ok(cx.undefined().upcast())
    }

  }
}