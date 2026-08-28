use neon::prelude::*;

mod primitives;
mod geometry;
mod colors;
mod text;
mod images;
mod paint;

pub use primitives::*;
pub use geometry::*;
pub use colors::*;
pub use text::*;
pub use images::*;
pub use paint::*;

//
// meta-helpers
//

fn arg_num(o:usize) -> String{
  // let n = (o + 1) as i32; // we're working with zero-bounded idxs
  let n = o; // arg 0 is always self, so no need to increment the idx
  let ords = ["st","nd","rd"];
  let slot = ((n+90)%100-10)%10 - 1;
  let suffix = if (0..=2).contains(&slot) { ords[slot] } else { "th" };
  format!("{}{}", n, suffix)
}

pub fn check_argc(cx: &mut FunctionContext, argc:usize) -> NeonResult<()>{
  match cx.len() >= argc {
    true => Ok(()),
    false => cx.throw_type_error("not enough arguments")
  }
}

// pub fn argv<'a>() -> Vec<Handle<'a, JsValue>>{
//   let list:Vec<Handle<JsValue>> = Vec::new();
//   list
// }

// pub fn symbol<'a>(cx: &mut FunctionContext<'a>, symbol_name: &str) -> JsResult<'a, JsValue> {
//   let global = cx.global();
//   let symbol_ctor = global
//       .get(cx, "Symbol")?
//       .downcast::<JsObject, _>(cx)
//       .or_throw(cx)?
//       .get(cx, "for")?
//       .downcast::<JsFunction, _>(cx)
//       .or_throw(cx)?;

//   let symbol_label = cx.string(symbol_name);
//   let sym = symbol_ctor.call(cx, global, vec![symbol_label])?;
//   Ok(sym)
// }

// Install a `process.once('exit', …)` listener that runs `on_exit` at termination
pub fn install_exit_handler(cx: &mut ModuleContext, on_exit: impl Fn() + 'static) -> NeonResult<()> {
  let process: Handle<JsObject> = cx.global("process")?;
  let once: Handle<JsFunction> = process.get(cx, "once")?;
  let handler = JsFunction::new(cx, move |mut cx| {
    on_exit();
    Ok(cx.undefined())
  })?;
  let event = cx.string("exit");
  once.call_with(cx).this(process).arg(event).arg(handler).exec(cx)?;
  Ok(())
}
