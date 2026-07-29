#![allow(non_camel_case_types)]

use neon::prelude::*;
use neon::types::buffer::TypedArray;
use skia_safe::{Rect, Point, Matrix, PathFillType};

use crate::path::BoxedPath2D;
use crate::context::BoxedContext2D;

// The encoder on the JS side accumulates verbs as `[opcode, args…]` records in a Float64Array
// and passes the whole buffer when it's time to draw. This module walks that buffer and converts
// the instructions into calls to the appropriate bezier methods on Context2D or Path2D. Path2D
// implements the `Pen` trait (the shared geometry verbs); Context2D implements `Plotter`, the
// subtrait that adds the context-only ops (state/transform/fill/stroke) on top of `Pen`.

#[repr(u32)]
#[derive(Clone, Copy)]
pub(crate) enum Op {
  // verbs shared by path & context
  moveTo, lineTo, bezierCurveTo, quadraticCurveTo, conicCurveTo,
  arc, arcTo, ellipse, rect, roundRect, closePath,
  // context-only verbs
  beginPath, save, restore, translate, scale, rotate, transform, setTransform,
  resetTransform, fill, stroke, fillRect, strokeRect, clearRect,
}

impl Op {
  const CODES:&[(Op, &str, usize)] = &[
    // variant             jsName,             arity
    (Op::moveTo,           "moveTo",           2),
    (Op::lineTo,           "lineTo",           2),
    (Op::bezierCurveTo,    "bezierCurveTo",    6),
    (Op::quadraticCurveTo, "quadraticCurveTo", 4),
    (Op::conicCurveTo,     "conicCurveTo",     5),
    (Op::arc,              "arc",              6),
    (Op::arcTo,            "arcTo",            5),
    (Op::ellipse,          "ellipse",          8),
    (Op::rect,             "rect",             4),
    (Op::roundRect,        "roundRect",        12),
    (Op::closePath,        "closePath",        0),
    (Op::beginPath,        "beginPath",        0),
    (Op::save,             "save",             0),
    (Op::restore,          "restore",          0),
    (Op::translate,        "translate",        2),
    (Op::scale,            "scale",            2),
    (Op::rotate,           "rotate",           1),
    (Op::transform,        "transform",        9),
    (Op::setTransform,     "setTransform",     9),
    (Op::resetTransform,   "resetTransform",   0),
    (Op::fill,             "fill",             1),
    (Op::stroke,           "stroke",           0),
    (Op::fillRect,         "fillRect",         4),
    (Op::strokeRect,       "strokeRect",       4),
    (Op::clearRect,        "clearRect",        4),
  ];

  // decode an opcode word from the buffer into an `Op`
  fn from_u32(op:u32) -> Option<Op> {
    Self::CODES.iter().find(|(v, _, _)| *v as u32 == op).map(|(v, _, _)| *v)
  }
  // number of f64 slots that follow this opcode in its slice of the buffer
  fn arity(self) -> usize {
    Self::CODES.iter().find(|(v, _, _)| *v as u32 == self as u32).map(|(_, _, a)| *a).unwrap_or(0)
  }
}

// shared line-drawing verbs (implemented by both Context2D & Path2D)
pub(crate) trait Pen {
  fn move_to(&mut self, x:f32, y:f32);
  fn line_to(&mut self, x:f32, y:f32);
  fn bezier_to(&mut self, c1x:f32, c1y:f32, c2x:f32, c2y:f32, x:f32, y:f32);
  fn quad_to(&mut self, cx:f32, cy:f32, x:f32, y:f32);
  fn conic_to(&mut self, cx:f32, cy:f32, x:f32, y:f32, w:f32);
  fn arc(&mut self, x:f32, y:f32, r:f32, start:f64, end:f64, ccw:bool);
  fn arc_to(&mut self, x1:f32, y1:f32, x2:f32, y2:f32, r:f32);
  fn ellipse(&mut self, x:f32, y:f32, xr:f32, yr:f32, rot:f64, start:f64, end:f64, ccw:bool);
  fn rect(&mut self, x:f32, y:f32, w:f32, h:f32);
  fn round_rect(&mut self, x:f32, y:f32, w:f32, h:f32, radii:[Point;4]);
  fn close(&mut self);

  // dispatch a single Pen opcode (return None if it's actually a Plotter op)
  fn plot_line(&mut self, op:Op, a:&[f64]) -> Option<()> {
    match op {
      Op::moveTo => self.move_to(a[0] as f32, a[1] as f32),
      Op::lineTo => self.line_to(a[0] as f32, a[1] as f32),
      Op::bezierCurveTo => self.bezier_to(a[0] as f32, a[1] as f32, a[2] as f32, a[3] as f32, a[4] as f32, a[5] as f32),
      Op::quadraticCurveTo => self.quad_to(a[0] as f32, a[1] as f32, a[2] as f32, a[3] as f32),
      Op::conicCurveTo => self.conic_to(a[0] as f32, a[1] as f32, a[2] as f32, a[3] as f32, a[4] as f32),
      Op::arc => self.arc(a[0] as f32, a[1] as f32, a[2] as f32, a[3], a[4], a[5] != 0.0),
      Op::arcTo => self.arc_to(a[0] as f32, a[1] as f32, a[2] as f32, a[3] as f32, a[4] as f32),
      Op::ellipse => self.ellipse(a[0] as f32, a[1] as f32, a[2] as f32, a[3] as f32, a[4], a[5], a[6], a[7] != 0.0),
      Op::rect => self.rect(a[0] as f32, a[1] as f32, a[2] as f32, a[3] as f32),
      Op::roundRect => {
        let radii = [
          Point::new(a[4] as f32, a[5] as f32), Point::new(a[6] as f32, a[7] as f32),
          Point::new(a[8] as f32, a[9] as f32), Point::new(a[10] as f32, a[11] as f32),
        ];
        self.round_rect(a[0] as f32, a[1] as f32, a[2] as f32, a[3] as f32, radii);
      }
      Op::closePath => self.close(),
      _ => return None, // not a Pen verb
    }
    Some(())
  }
}

// extra verbs only supported by Context2D
pub(crate) trait Plotter: Pen {
  fn begin_path(&mut self);
  fn save(&mut self);
  fn restore(&mut self);
  fn translate(&mut self, x:f32, y:f32);
  fn scale(&mut self, x:f32, y:f32);
  fn rotate(&mut self, radians:f64);
  fn transform(&mut self, matrix:Matrix);
  fn set_transform(&mut self, matrix:Matrix);
  fn reset_transform(&mut self);
  fn fill(&mut self, rule:PathFillType);
  fn stroke(&mut self);
  fn fill_rect(&mut self, rect:Rect);
  fn stroke_rect(&mut self, rect:Rect);
  fn clear_rect(&mut self, rect:Rect);

  // dispatch a single Plotter *or* Pen opcode
  fn plot(&mut self, op:Op, a:&[f64]) -> Result<(), String> {
    // first, try handing off to the Pen-verb dispatcher...
    if self.plot_line(op, a).is_none() {
      // ...but if it's Plotter-specific, dispatch manually
      match op {
        Op::beginPath      => self.begin_path(),
        Op::save           => self.save(),
        Op::restore        => self.restore(),
        Op::translate      => self.translate(a[0] as f32, a[1] as f32),
        Op::scale          => self.scale(a[0] as f32, a[1] as f32),
        Op::rotate         => self.rotate(a[0]),
        Op::transform      => self.transform(mat9(a)),
        Op::setTransform   => self.set_transform(mat9(a)),
        Op::resetTransform => self.reset_transform(),
        Op::fill           => self.fill(if a[0] == 1.0 { PathFillType::EvenOdd } else { PathFillType::Winding }),
        Op::stroke         => self.stroke(),
        Op::fillRect       => self.fill_rect(Rect::from_xywh(a[0] as f32, a[1] as f32, a[2] as f32, a[3] as f32)),
        Op::strokeRect     => self.stroke_rect(Rect::from_xywh(a[0] as f32, a[1] as f32, a[2] as f32, a[3] as f32)),
        Op::clearRect      => self.clear_rect(Rect::from_xywh(a[0] as f32, a[1] as f32, a[2] as f32, a[3] as f32)),
        _ => return Err(format!("DrawList opcode unknown: {}", op as u32)),
      }
    }
    Ok(())
  }
}

// build a 3x3 matrix from the 9-term toSkMatrix ordering ([a,c,e, b,d,f, m14,m24,m44])
fn mat9(a:&[f64]) -> Matrix {
  Matrix::new_all(
    a[0] as f32, a[1] as f32, a[2] as f32,
    a[3] as f32, a[4] as f32, a[5] as f32,
    a[6] as f32, a[7] as f32, a[8] as f32,
  )
}

// parses the buffer contents and provides an iterator over the decoded Op records
struct DrawList<'a> { data: &'a [f64] }

impl<'a> DrawList<'a> {
  fn from(buffer: &'a [f64], len: usize) -> Self {
    DrawList { data: &buffer[..len.min(buffer.len())] }
  }
}

impl<'a> Iterator for DrawList<'a> {
  type Item = Result<(Op, &'a [f64]), String>;
  fn next(&mut self) -> Option<Self::Item> {
    loop {
      let (&head, rest) = self.data.split_first()?; // returns None if complete
      let raw = head as u32;
      let Some(op) = Op::from_u32(raw) else {
        return Some(Err(format!("DrawList opcode unknown: {raw}")));
      };
      let Some((args, tail)) = rest.split_at_checked(op.arity()) else {
        return Some(Err(format!("DrawList contains truncated entry for opcode {raw}")));
      };
      self.data = tail;
      // only dispatch if all args are valid numbers (taking the isFinite check off js's shoulders)
      if args.iter().all(|n| n.is_finite()) { return Some(Ok((op, args))); }
    }
  }
}

//
// -- Javascript Methods --------------------------------------------------------------------------
//

// Drawlist_opcodes() → { name: {op, arity}, … } — read once at JS startup.
pub fn opcodes(mut cx: FunctionContext) -> JsResult<JsObject> {
  let obj = cx.empty_object();
  for (variant, name, ar) in Op::CODES {
    let entry = cx.empty_object();
    let op_num = cx.number(*variant as u32 as f64);
    let ar_num = cx.number(*ar as f64);
    entry.set(&mut cx, "op", op_num)?;
    entry.set(&mut cx, "arity", ar_num)?;
    let key = cx.string(*name);
    obj.set(&mut cx, key, entry)?;
  }
  Ok(obj)
}

// {Path2D,CanvasRenderingContext2D}_plot(this, buffer, len) — decode the first `len`
// slots of `buffer` into Ops and dispatch them on `this` (a Context2D or Path2D)
pub fn plot(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<JsValue>(0)?;
  let buffer = cx.argument::<JsFloat64Array>(1)?;
  let len = cx.argument::<JsNumber>(2)?.value(&mut cx) as usize;

  // Resolve which boxed type `this` is *before* borrowing the buffer slice:
  // downcast needs `&mut cx`, while `as_slice` holds a shared `&cx`.
  let path = this.downcast::<BoxedPath2D, _>(&mut cx).ok();
  let ctx  = this.downcast::<BoxedContext2D, _>(&mut cx).ok();

  let result = {
    let mut drawlist = DrawList::from(buffer.as_slice(&cx), len);
    let mut ctx  = ctx.as_ref().map(|c| c.borrow_mut());   // Option<RefMut<Context2D>>
    let mut path = path.as_ref().map(|p| p.borrow_mut());  // Option<RefMut<Path2D>>

    // decode & dispatch each Op in the buffer
    drawlist.try_for_each(|rec| {
      let (op, args) = rec?; // bail out if there was a decoding error
      match (ctx.as_mut(), path.as_mut()) {
        (Some(ctx), _) => ctx.plot(op, args),
        (_, Some(path)) => path.plot_line(op, args)
          .ok_or_else(|| format!("DrawList opcode {} is not a Path2D verb", op as u32)),
        _ => Err("DrawList target is neither a Path2D nor a Context2D".to_string()),
      }
    })
  };

  match result {
    Ok(()) => Ok(cx.undefined()),
    Err(msg) => cx.throw_error(msg),
  }
}
