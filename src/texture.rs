#![allow(unused_variables)]
#![allow(unused_mut)]
#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(non_snake_case)]
use std::cell::RefCell;
use std::sync::{Arc, Mutex};
use std::f32::consts::PI;
use neon::prelude::*;
use skia_safe::{Path, Rect, Color, Color4f, Point, TileMode, Matrix, Paint, PaintStyle};
use skia_safe::{PathEffect, line_2d_path_effect, path_2d_path_effect};

use crate::utils::*;
use crate::path::BoxedPath2D;

#[derive(Debug)]
struct Texture{
  path: Option<Path>,
  color: Color,
  line: f32,
  angle: f32,
  scale: (f32, f32),
  shift: (f32, f32),
}

pub type BoxedCanvasTexture = JsBox<RefCell<CanvasTexture>>;
impl Finalize for CanvasTexture {}

impl Default for Texture {
  fn default() -> Self {
    Texture{path:None, color:Color::BLACK, line:1.0, angle:0.0, scale:(1.0, 1.0), shift:(0.0, 0.0)}
  }
}

#[derive(Clone)]
pub struct CanvasTexture{
  texture:Arc<Mutex<Texture>>
}

impl CanvasTexture{
  pub fn mix_into(&self, paint: &mut Paint, alpha:f32){
    let tile = Arc::clone(&self.texture);
    let tile = tile.lock().unwrap();

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
      paint.set_style(PaintStyle::Stroke);
    }

    let mut color:Color4f = tile.color.into();
    color.a *= alpha;
    paint.set_color(color.to_color());
  }

  pub fn spacing(&self) -> (f32, f32) {
    let tile = Arc::clone(&self.texture);
    let tile = tile.lock().unwrap();
    tile.scale
  }

  pub fn to_color(&self, alpha:f32) -> Color {
    let tile = Arc::clone(&self.texture);
    let tile = tile.lock().unwrap();

    let mut color:Color4f = tile.color.into();
    color.a *= alpha;
    color.to_color()
  }

}

//
// -- Javascript Methods --------------------------------------------------------------------------
//

pub fn new(mut cx: FunctionContext) -> JsResult<BoxedCanvasTexture> {
  let path = opt_path2d_arg(&mut cx, 1);
  let color = color_arg(&mut cx, 2).unwrap_or(Color::BLACK);
  let line = float_arg(&mut cx, 3, "line")?;
  let nums = float_args(&mut cx, 4..9)?;

  let texture = match nums.as_slice(){
    [angle, h, v, x, y] => {
      let angle = *angle;
      let scale = (*h, *v);
      let shift = (*x, *y);
      Texture{path, color, line, angle, scale, shift}
    },
    _ => Texture::default()
  };

  let canvas_texture = CanvasTexture{ texture:Arc::new(Mutex::new(texture)) };
  let this = RefCell::new(canvas_texture);
  Ok(cx.boxed(this))
}

pub fn repr(mut cx: FunctionContext) -> JsResult<JsString> {
  let this = cx.argument::<BoxedCanvasTexture>(0)?;
  let this = this.borrow();

  let tile = Arc::clone(&this.texture);
  let tile = tile.lock().unwrap();

  let style = if tile.path.is_some(){ "Path" }else{ "Lines" };
  Ok(cx.string(style))
}