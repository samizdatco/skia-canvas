use neon::prelude::*;

mod context2d;
mod path2d;
mod gradient;
mod utils;

register_module!(mut m, {
  m.export_class::<crate::context2d::JsContext2D>("CanvasRenderingContext2D")?;
  m.export_class::<crate::gradient::JsCanvasGradient>("CanvasGradient")?;
  m.export_class::<crate::path2d::JsPath2D>("Path2D")?;
  Ok(())
});
