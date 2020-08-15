use neon::prelude::*;

mod canvas;
mod context;
mod path;
mod image;
mod gradient;
mod pattern;
mod utils;

register_module!(mut m, {
  m.export_class::<crate::canvas::JsCanvas>("Canvas")?;
  m.export_class::<crate::context::JsContext2D>("CanvasRenderingContext2D")?;
  m.export_class::<crate::gradient::JsCanvasGradient>("CanvasGradient")?;
  m.export_class::<crate::pattern::JsCanvasPattern>("CanvasPattern")?;
  m.export_class::<crate::path::JsPath2D>("Path2D")?;
  m.export_class::<crate::image::JsImage>("Image")?;
  m.export_class::<crate::image::JsImageData>("ImageData")?;
  m.export_class::<crate::utils::JsFontLibrary>("FontLibrary")?;
  Ok(())
});
