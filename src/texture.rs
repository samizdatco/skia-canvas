use std::cell::RefCell;
use std::rc::Rc;
use std::f32::consts::PI;
use neon::prelude::*;
use skia_safe::{Path, PathBuilder, Color4f, Matrix, Paint, PaintStyle, PaintCap, Point, Rect};
use skia_safe::{line_2d_path_effect, path_2d_path_effect, path_utils::fill_path_with_paint};

use crate::bridge::*;

struct Texture{
  path: Option<Path>,
  color: Color4f,
  line: f32,
  cap: PaintCap,
  angle: f32,
  scale: (f32, f32),
  shift: (f32, f32),
}

impl Texture{
  // maps lattice coordinates (one unit per tile) to the canvas
  fn lattice_matrix(&self) -> Matrix{
    let scale = match self.path{
      Some(_) => self.scale,
      None => { let s = self.scale.0.max(self.scale.1); (s, s) }
    };
    let mut matrix = Matrix::new_identity();
    matrix
      .pre_translate(self.shift)
      .pre_rotate(180.0 * self.angle / PI, None)
      .pre_scale(scale, None);
    matrix
  }

  fn lattice_aligned_path(&self) -> Option<Path>{
    self.path.as_ref().map(|path| path.with_transform(&Matrix::rotate_rad(self.angle)))
  }
}

pub type BoxedCanvasTexture = JsBox<RefCell<CanvasTexture>>;
impl Finalize for CanvasTexture {}

impl Default for Texture {
  fn default() -> Self {
    Texture{path:None, color:Color4f::new(0.0, 0.0, 0.0, 1.0), line:1.0, cap:PaintCap::Butt, angle:0.0, scale:(1.0, 1.0), shift:(0.0, 0.0)}
  }
}

#[derive(Clone)]
pub struct CanvasTexture{
  texture:Rc<RefCell<Texture>>,
  outline: bool,
}

impl CanvasTexture{
  pub fn mix_into(&self, paint: &mut Paint, alpha:f32){
    let tile = self.texture.borrow();
    let matrix = tile.lattice_matrix();
    match tile.lattice_aligned_path() {
      Some(path) => paint.set_path_effect(path_2d_path_effect::new(&matrix, &path)),
      None => paint.set_path_effect(line_2d_path_effect::new(tile.line, &matrix)),
    };

    if tile.line > 0.0{
      paint.set_stroke_width(tile.line);
      paint.set_stroke_cap(tile.cap);
      paint.set_style(PaintStyle::Stroke);
    }else{
      paint.set_style(PaintStyle::Fill);
    }

    let mut color = tile.color;
    color.a *= alpha;
    paint.set_color4f(color, None);
  }

  pub fn use_clip(&self) -> bool{
    !self.outline
  }

  // the area the path effect must fill so that every tile overlapping `stencil` gets drawn
  // (since a cell's tile is only drawn if the cell's center falls inside it)
  pub fn frame(&self, stencil:&Path, tile_paint:&Paint, matrix:&Matrix) -> Path{
    let tile = self.texture.borrow();
    let bounds = stencil.bounds();
    if bounds.is_empty(){ return Path::default() }

    // anti-aliasing extends about one device pixel past the tile's geometry
    let pad = match matrix.min_scale(){ scale if scale > 0.0 => 1.0 / scale, _ => 1.0 };

    // the extent of one cell's geometry relative to its center
    let reach = match tile.lattice_aligned_path(){
      Some(path) => {
        let mut traced = PathBuilder::new();
        let mut plain = tile_paint.clone();
        plain.set_path_effect(None);
        fill_path_with_paint(&path, &plain, &mut traced, None, None);
        traced.detach().bounds().with_outset((pad, pad)).to_quad(None)
      }
      None => {
        // stripes run between cell centers and skip rows of a single cell, so the reach extends a full cell each way
        let cell = tile.lattice_matrix().map_vector((1.0, 0.0)).length();
        let length = cell + pad + if tile.cap == PaintCap::Butt { 0.0 } else { tile.line / 2.0 };
        let width = tile.line / 2.0 + pad;
        Rect::new(-length, -width, length, width).to_quad(None).map(|pt| Matrix::rotate_rad(tile.angle).map_point(pt))
      }
    };

    // the convex hull of the bounds' corners shifted by the reach's corners (via Andrew's monotone chain)
    let hull = {
      let mut points:Vec<Point> = bounds.to_quad(None).into_iter()
        .flat_map(|pt| reach.iter().map(move |offset| pt - *offset))
        .collect();
      points.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
      let cross = |o:Point, a:Point, b:Point| (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x);
      let half = |points:&mut dyn Iterator<Item=&Point>| {
        let mut chain:Vec<Point> = vec![];
        for &pt in points{
          while chain.len() >= 2 && cross(chain[chain.len() - 2], chain[chain.len() - 1], pt) <= 0.0 { chain.pop(); }
          chain.push(pt);
        }
        chain.pop();
        chain
      };
      let mut hull = half(&mut points.iter());
      hull.extend(half(&mut points.iter().rev()));
      hull
    };
    let mut frame = PathBuilder::new();
    frame.add_polygon(&hull, true);
    frame.detach()
  }

  pub fn to_color_4f(&self, alpha:f32) -> Color4f {
    let tile = self.texture.borrow();
    let mut color = tile.color;
    color.a *= alpha;
    color
  }

}

//
// -- Javascript Methods --------------------------------------------------------------------------
//

pub fn new(mut cx: FunctionContext) -> JsResult<BoxedCanvasTexture> {
  let path = opt_skpath_arg(&mut cx, 1);
  let color = opt_color_arg_4f(&mut cx, 2).unwrap_or(Color4f::new(0.0, 0.0, 0.0, 1.0));

  let line = match opt_float_arg(&mut cx, 3){
    Some(weight) => weight,
    None => cx.throw_type_error("Expected a number for `line`")?
  };

  let cap = match to_stroke_cap(&string_arg(&mut cx, 4, "cap")?){
    Some(style) => style,
    None => cx.throw_type_error("Expected \"butt\", \"square\", or \"round\" for `cap`")?
  };

  let angle = match opt_float_arg(&mut cx, 5){
    Some(theta) => theta,
    None => cx.throw_type_error("Expected a number for `angle`")?
  };

  let outline = bool_arg(&mut cx, 6, "outline")?;

  let scale = match opt_float_args(&mut cx, 7..9).as_slice(){
    [h, v] => (*h, *v),
    _ => cx.throw_type_error("Expected a number or array with 2 numbers for `spacing`")?
  };

  let shift = match opt_float_args(&mut cx, 9..11).as_slice(){
    [h, v] => (*h, *v),
    _ => cx.throw_type_error("Expected a number or array with 2 numbers for `offset`")?
  };

  let texture = Texture{path, color, line, cap, angle, scale, shift};
  let canvas_texture = CanvasTexture{ texture:Rc::new(RefCell::new(texture)), outline };
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
