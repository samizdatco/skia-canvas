#![allow(non_snake_case)]
use std::cell::RefCell;
use neon::prelude::*;
use skia_safe::SurfaceProps;
use serde_json::json;
use crate::utils::*;
use crate::context::page::{ExportOptions, pages_arg};
use crate::gpu;

pub type BoxedCanvas = JsBox<RefCell<Canvas>>;
impl Finalize for Canvas {}

pub struct Canvas{
  pub width: f32,
  pub height: f32,
  pub text_contrast: f64,
  pub text_gamma: f64,
  engine: Option<gpu::RenderingEngine>,
}

impl Canvas{
  pub fn new(text_contrast:f64, text_gamma:f64) -> Self{
    Canvas{width:300.0, height:150.0, text_contrast, text_gamma, engine:None}
  }

  pub fn engine(&mut self) -> gpu::RenderingEngine{
    self.engine.get_or_insert_with(||
      gpu::RenderingEngine::default()
    ).clone()
  }

  pub fn export_options(&self) -> ExportOptions{
    ExportOptions{text_contrast:self.text_contrast as _, text_gamma:self.text_gamma as _, ..Default::default()}
  }
}

//
// -- Javascript Methods --------------------------------------------------------------------------
//

pub fn new(mut cx: FunctionContext) -> JsResult<BoxedCanvas> {
  let opts = cx.argument::<JsObject>(1)?;
  let text_contrast = opt_double_for_key(&mut cx, &opts, "textContrast").unwrap_or(0.0);
  let (min_c, max_c) = (SurfaceProps::MIN_CONTRAST_INCLUSIVE as _, SurfaceProps::MAX_CONTRAST_INCLUSIVE as _);
  if text_contrast < min_c || text_contrast > max_c{
    return cx.throw_range_error(format!("Expected a number between {} and {} for `textContrast`", min_c, max_c))
  }

  let mut text_gamma = opt_double_for_key(&mut cx, &opts, "textGamma").unwrap_or(1.4);
  let (min_g, max_g) = (SurfaceProps::MIN_GAMMA_INCLUSIVE as _, SurfaceProps::MAX_GAMMA_EXCLUSIVE as _);
  if text_gamma == max_g{ text_gamma -= f32::EPSILON as f64 }; // nudge down values right at the max
  if text_gamma < min_g || text_contrast > max_g{
    return cx.throw_range_error(format!("Expected a number between {} and {} for `textGamma`", min_g, max_g))
  }

  let this = RefCell::new(Canvas::new(text_contrast as f64, text_gamma as f64));
  Ok(cx.boxed(this))
}

pub fn get_width(mut cx: FunctionContext) -> JsResult<JsNumber> {
  let this = cx.argument::<BoxedCanvas>(0)?;
  let width = this.borrow().width;
  Ok(cx.number(width as f64))
}

pub fn get_height(mut cx: FunctionContext) -> JsResult<JsNumber> {
  let this = cx.argument::<BoxedCanvas>(0)?;
  let height = this.borrow().height;
  Ok(cx.number(height as f64))
}

pub fn set_width(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedCanvas>(0)?;
  let width = float_arg_or_bail(&mut cx, 1, "size")?;
  if width < 0.0{
    cx.throw_range_error("⚠️Dimensions must be non-zero")?
  }
  this.borrow_mut().width = width;
  Ok(cx.undefined())
}

pub fn set_height(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedCanvas>(0)?;
  let height = float_arg_or_bail(&mut cx, 1, "size")?;
  if height < 0.0{
    cx.throw_range_error("⚠️Dimensions must be non-zero")?
  }
  this.borrow_mut().height = height;
  Ok(cx.undefined())
}

pub fn get_engine(mut cx: FunctionContext) -> JsResult<JsString> {
  let this = cx.argument::<BoxedCanvas>(0)?;
  let mut this = this.borrow_mut();
  Ok(cx.string(from_engine(this.engine())))
}

pub fn set_engine(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedCanvas>(0)?;
  if let Some(engine_name) = opt_string_arg(&mut cx, 1){
    if let Some(new_engine) = to_engine(&engine_name){
      if new_engine.selectable() {
        this.borrow_mut().engine = Some(new_engine)
      }
    }
  }

  Ok(cx.undefined())
}

pub fn get_engine_status(mut cx: FunctionContext) -> JsResult<JsString> {
  let this = cx.argument::<BoxedCanvas>(0)?;
  let mut this = this.borrow_mut();

  let mut details = this.engine().status();
  details["textContrast"] = json!(this.text_contrast);
  details["textGamma"] = json!(this.text_gamma);
  Ok(cx.string(details.to_string()))
}

pub fn toBuffer(mut cx: FunctionContext) -> JsResult<JsPromise> {
  let this = cx.argument::<BoxedCanvas>(0)?;
  let options = export_options_arg(&mut cx, 2)?;
  let mut pages = pages_arg(&mut cx, 1, &options, &this)?;

  // ensure cached bitmaps are sendable to other thread
  pages.materialize(&this.borrow_mut().engine(), &options);

  let channel = cx.channel();
  let (deferred, promise) = cx.promise();
  rayon::spawn_fifo(move || {
    let result = {
      if options.format=="pdf" && pages.len() > 1 {
        pages.as_pdf(options)
      }else{
        pages.first().encoded_as(options, pages.engine)
      }
    };

    deferred.settle_with(&channel, move |mut cx| {
      let data = result.or_else(|err| cx.throw_error(err))?;
      let buffer = JsBuffer::from_slice(&mut cx, &data)?;
      Ok(buffer)
    });
  });

  Ok(promise)
}

pub fn toBufferSync(mut cx: FunctionContext) -> JsResult<JsValue> {
  let this = cx.argument::<BoxedCanvas>(0)?;
  let options = export_options_arg(&mut cx, 2)?;
  let pages = pages_arg(&mut cx, 1, &options, &this)?;

  let encoded = {
    if options.format=="pdf" && pages.len() > 1 {
      pages.as_pdf(options)
    }else{
      pages.first().encoded_as(options, pages.engine)
    }
  };

  match encoded{
    Ok(data) => {
      let buffer = JsBuffer::from_slice(&mut cx, &data)?;
      Ok(buffer.upcast::<JsValue>())
    },
    Err(msg) => cx.throw_error(msg)
  }
}

pub fn save(mut cx: FunctionContext) -> JsResult<JsPromise> {
  let this = cx.argument::<BoxedCanvas>(0)?;
  let name_pattern = string_arg(&mut cx, 2, "filePath")?;
  let sequence = !cx.argument::<JsValue>(3)?.is_a::<JsUndefined, _>(&mut cx);
  let padding = opt_float_arg(&mut cx, 3).unwrap_or(-1.0);
  let options = export_options_arg(&mut cx, 4)?;
  let mut pages = pages_arg(&mut cx, 1, &options, &this)?;

  // ensure cached bitmaps are sendable to other thread
  pages.materialize(&this.borrow_mut().engine(), &options);

  let channel = cx.channel();
  let (deferred, promise) = cx.promise();
  rayon::spawn_fifo(move || {
    let result = {
      if sequence {
        pages.write_sequence(&name_pattern, padding, options)
      } else if options.format == "pdf" {
        pages.write_pdf(&name_pattern, options)
      } else {
        pages.write_image(&name_pattern, options)
      }
    };

    deferred.settle_with(&channel, move |mut cx| match result{
      Err(msg) => cx.throw_error(format!("I/O Error: {}", msg)),
      _ => Ok(cx.undefined())
    });
  });

  Ok(promise)
}

pub fn saveSync(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedCanvas>(0)?;
  let name_pattern = string_arg(&mut cx, 2, "filePath")?;
  let sequence = !cx.argument::<JsValue>(3)?.is_a::<JsUndefined, _>(&mut cx);
  let padding = opt_float_arg(&mut cx, 3).unwrap_or(-1.0);
  let options = export_options_arg(&mut cx, 4)?;
  let pages = pages_arg(&mut cx, 1, &options, &this)?;

  let result = {
    if sequence {
      pages.write_sequence(&name_pattern, padding, options)
    } else if options.format == "pdf" {
      pages.write_pdf(&name_pattern, options)
    } else {
      pages.write_image(&name_pattern, options)
    }
  };

  match result{
    Ok(_) => Ok(cx.undefined()),
    Err(msg) => cx.throw_error(msg)
  }
}
