#![allow(non_snake_case)]
use std::cell::RefCell;
use std::rc::Rc;
use std::f32::consts::PI;
use neon::prelude::*;
use skia_safe::{Path, Color, Color4f, Matrix, Paint, PaintStyle, PaintCap};
use skia_safe::{line_2d_path_effect, path_2d_path_effect};

use crate::utils::*;

struct Texture{
  path: Option<Path>,
  color: Color,
  line: f32,
  cap: PaintCap,
  angle: f32,
  scale: (f32, f32),
  shift: (f32, f32),
}

pub type BoxedCanvasTexture = JsBox<RefCell<CanvasTexture>>;
impl Finalize for CanvasTexture {}

impl Default for Texture {
  fn default() -> Self {
    Texture{path:None, color:Color::BLACK, line:1.0, cap:PaintCap::Butt, angle:0.0, scale:(1.0, 1.0), shift:(0.0, 0.0)}
  }
}

#[derive(Clone)]
pub struct CanvasTexture{
  texture:Rc<RefCell<Texture>>
}

impl CanvasTexture{
  pub fn mix_into(&self, paint: &mut Paint, alpha:f32){
    let tile = self.texture.borrow();

    let mut matrix = Matrix::new_identity();
    matrix
      .pre_translate(tile.shift)
      .pre_rotate(180.0 * tile.angle / PI, None);

    match &tile.path {
      Some(path) => {
        let path = path.with_transform(&Matrix::rotate_rad(tile.angle));
        matrix.pre_scale(tile.scale, None);
        paint.set_path_effect(path_2d_path_effect::new(&matrix, &path));
      }
      None => {
        let scale = tile.scale.0.max(tile.scale.1);
        matrix.pre_scale((scale, scale), None);
        paint.set_path_effect(line_2d_path_effect::new(tile.line, &matrix));
      }
    };

    if tile.line > 0.0{
      paint.set_stroke_width(tile.line);
      paint.set_stroke_cap(tile.cap);
      paint.set_style(PaintStyle::Stroke);
    }

    let mut color:Color4f = tile.color.into();
    color.a *= alpha;
    paint.set_color(color.to_color());
  }

  pub fn spacing(&self) -> (f32, f32) {
    let tile = self.texture.borrow();
    tile.scale
  }

  pub fn to_color(&self, alpha:f32) -> Color {
    let tile = self.texture.borrow();
    let mut color:Color4f = tile.color.into();
    color.a *= alpha;
    color.to_color()
  }

}

//
// -- Javascript Methods --------------------------------------------------------------------------
//

pub fn new(mut cx: FunctionContext) -> JsResult<BoxedCanvasTexture> {
  let path = opt_skpath_arg(&mut cx, 1);
  let color = opt_color_arg(&mut cx, 2).unwrap_or(Color::BLACK);

  let line = match opt_float_arg(&mut cx, 3){
    Some(weight) => weight,
    None => cx.throw_type_error("Expected a number for `line`")?
  };

  let cap = match opt_string_arg(&mut cx, 4).or(Some("butt".to_string())).and_then(|c| to_stroke_cap(&c)){
    Some(style) => style,
    None => cx.throw_type_error("Expected \"butt\", \"square\", or \"round\" for `cap`")?
  };

  let angle = match opt_float_arg(&mut cx, 5){
    Some(theta) => theta,
    None => cx.throw_type_error("Expected a number for `angle`")?
  };

  let scale = match opt_float_args(&mut cx, 6..8).as_slice(){
    [h, v] => (*h, *v),
    _ => cx.throw_type_error("Expected a number or array with 2 numbers for `spacing`")?
  };

  let shift = match opt_float_args(&mut cx, 8..10).as_slice(){
    [h, v] => (*h, *v),
    _ => cx.throw_type_error("Expected a number or array with 2 numbers for `offset`")?
  };

  let texture = Texture{path, color, line, cap, angle, scale, shift};
  let canvas_texture = CanvasTexture{ texture:Rc::new(RefCell::new(texture)) };
  let this = RefCell::new(canvas_texture);
  Ok(cx.boxed(this))
}

pub fn repr(mut cx: FunctionContext) -> JsResult<JsString> {
  let this = cx.argument::<BoxedCanvasTexture>(0)?;
  let this = this.borrow();

  let tile = this.texture.borrow();
  let style = if tile.path.is_some(){ "Path" }else{ "Lines" };
  Ok(cx.string(style))
}