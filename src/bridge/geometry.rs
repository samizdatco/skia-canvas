use std::f32::consts::PI;
use neon::prelude::*;
use skia_safe::{Path, Matrix, Point, Rect};

use super::{arg_num, floats_in};

//
// Math
//

#[allow(dead_code)] // kept as the counterpart to almost_zero
pub fn almost_equal(a: f32, b: f32) -> bool{
  (a-b).abs() < 0.00001
}

pub fn almost_zero(a: f32) -> bool{
  a.abs() < 0.00001
}

pub fn to_degrees(radians: f32) -> f32{
  radians / PI * 180.0
}

pub fn to_radians(degrees: f32) -> f32{
  degrees / 180.0 * PI
}

// pub fn clamp(val: f32, min:f64, max:f64) -> f32{
//   let min = min as f32;
//   let max = max as f32;
//   if val < min { min } else if val > max { max } else { val }
// }

// build a canvas spec-conforming Rect by normalizing negative width/height dimensions
// into a sorted (left ≤ right, top ≤ bottom) rectangle
pub fn normalized_rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
  Rect::from_ltrb(x.min(x + w), y.min(y + h), x.max(x + w), y.max(y + h))
}

//
// Matrices
//

pub fn to_matrix(t:&[f32]) -> Option<Matrix>{
  match t.len(){
    6 => Some(Matrix::new_all(t[0], t[1], t[2], t[3], t[4], t[5], 0.0, 0.0, 1.0)),
    9 => Some(Matrix::new_all(t[0], t[1], t[2], t[3], t[4], t[5], t[6], t[7], t[8])),
    _ => None
  }
}

// pub fn matrix_in(cx: &mut FunctionContext, vals:&[Handle<JsValue>]) -> NeonResult<Matrix>{
//   // for converting single js-array args
//   let terms = floats_in(vals);
//   match to_matrix(&terms){
//     Some(matrix) => Ok(matrix),
//     None => cx.throw_error(format!("expected 6 or 9 matrix values (got {})", terms.len()))
//   }
// }

// pub fn matrix_args(cx: &mut FunctionContext, rng: Range<usize>) -> NeonResult<Matrix>{
//   // for converting inline args (e.g., in Path.transform())
//   let terms = opt_float_args(cx, rng);
//   match to_matrix(&terms){
//     Some(matrix) => Ok(matrix),
//     None => cx.throw_error(format!("expected 6 or 9 matrix values (got {})", terms.len()))
//   }
// }

pub fn opt_matrix_arg(cx: &mut FunctionContext, idx: usize) -> Option<Matrix>{
  if let Some(arg) = cx.argument_opt(idx) {
    if let Ok(array) = arg.downcast::<JsArray, _>(cx) {
      if let Ok(vals) = array.to_vec(cx){
        let terms = floats_in(cx, &vals);
        return to_matrix(&terms)
      }
    }
  }
  None
}

pub fn matrix_arg(cx: &mut FunctionContext, idx:usize) -> NeonResult<Matrix> {
  match opt_matrix_arg(cx, idx){
    Some(v) => Ok(v),
    None => cx.throw_type_error("Expected a DOMMatrix")
  }
}

//
// Points
//

pub fn points_arg(cx: &mut FunctionContext, idx: usize) -> NeonResult<Vec<Point>>{
  let mut nums:Vec<f32> = vec![];
  if let Some(arg) = cx.argument_opt(idx) {
    if let Ok(array) = arg.downcast::<JsArray, _>(cx) {
      if let Ok(vals) = array.to_vec(cx){
        nums = floats_in(cx, &vals);
      }
    }
  }

  if nums.len() % 2 == 1{
    let which = if idx==1{ "first" }else if idx==2{ "second" }else{ "an" };
    cx.throw_type_error(
      format!("Lists of x/y points must have an even number of values (got {} in {} argument)", nums.len(), which)
    )
  }else{
    let points = nums
      .as_slice()
      .chunks_exact(2)
      .map(|pair| Point::new(pair[0], pair[1]))
      .collect();
    Ok(points)
  }
}

//
// Path2D
//

use crate::path::{BoxedPath2D};

pub fn opt_skpath_arg(cx: &mut FunctionContext, idx:usize) -> Option<Path> {
  if let Some(arg) = cx.argument_opt(idx){
    if let Ok(arg) = arg.downcast::<BoxedPath2D, _>(cx){
      let arg = arg.borrow();
      return Some(arg.path())
    }
  }
  None
}

pub fn path2d_arg<'a>(cx: &mut FunctionContext<'a>, idx: usize) -> NeonResult<Handle<'a, BoxedPath2D>>{
  if cx.len() <= idx {
    return cx.throw_type_error(format!("not enough arguments (missing: Path2D as {} arg)", arg_num(idx)));
  }

  match cx.argument::<JsValue>(idx)?.downcast::<BoxedPath2D, _>(cx){
    Ok(path_obj) => Ok(path_obj),
    Err(_) => cx.throw_type_error(format!("Expected a Path2D for {} arg", arg_num(idx)))
  }
}

use skia_safe::{PathOp};
pub fn to_path_op(op_name:&str) -> Option<PathOp> {
  let op = match op_name.to_lowercase().as_str() {
    "difference" => PathOp::Difference,
    "intersect" => PathOp::Intersect,
    "union" => PathOp::Union,
    "xor" => PathOp::XOR,
    "reversedifference" | "complement" => PathOp::ReverseDifference,
    _ => return None
  };
  Some(op)
}

use skia_safe::path_1d_path_effect;
pub fn to_1d_style(mode_name:&str) -> Option<path_1d_path_effect::Style>{
  let mode = match mode_name.to_lowercase().as_str(){
    "move" => path_1d_path_effect::Style::Translate,
    "turn" => path_1d_path_effect::Style::Rotate,
    "follow" => path_1d_path_effect::Style::Morph,
    _ => return None
  };
  Some(mode)
}

pub fn from_1d_style(mode:path_1d_path_effect::Style) -> String{
  match mode{
    path_1d_path_effect::Style::Translate => "move",
    path_1d_path_effect::Style::Rotate => "turn",
    path_1d_path_effect::Style::Morph => "follow"
  }.to_string()
}

use skia_safe::PathFillType;

pub fn fill_rule_arg_or(cx: &mut FunctionContext, idx: usize, default: &str) -> NeonResult<PathFillType>{
  let err_msg = format!("Expected `fillRule` to be \"nonzero\" or \"evenodd\" for {} arg", arg_num(idx));

  // if arg is provided, verify that it's a string (if absent use default val). An explicit
  // `undefined` is treated as a missing optional arg per WebIDL — so clip(undefined) / fill(undefined)
  // / isPointInPath(…, undefined) fall back to the default rule rather than throwing. (Verified
  // against Chrome: `undefined` behaves as the default "nonzero"; `null` and bogus strings throw.)
  let mode = match cx.argument_opt(idx) {
    Some(arg) if arg.is_a::<JsUndefined, _>(cx) => Ok(default.to_string()),
    Some(arg) => match arg.downcast::<JsString, _>(cx) {
      Ok(v) => Ok(v.value(cx)),
      Err(_e) => cx.throw_type_error(&err_msg)
    },
    None => Ok(default.to_string())
  }?;


  match mode.as_str(){
    "nonzero" => Ok(PathFillType::Winding),
    "evenodd" => Ok(PathFillType::EvenOdd),
    _ => cx.throw_type_error(&err_msg)
  }
}
