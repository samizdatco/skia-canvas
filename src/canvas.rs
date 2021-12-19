#![allow(non_snake_case)]
use std::cell::RefCell;
use neon::prelude::*;

use crate::utils::*;
use crate::context::page::pages_arg;

pub type BoxedCanvas = JsBox<RefCell<Canvas>>;
impl Finalize for Canvas {}

pub struct Canvas{
  pub width: f32,
  pub height: f32,
  async_io: bool,
}

impl Canvas{
  pub fn new() -> Self{
    Canvas{width:300.0, height:150.0, async_io:true}
  }
}

//
// -- Javascript Methods --------------------------------------------------------------------------
//

pub fn new(mut cx: FunctionContext) -> JsResult<BoxedCanvas> {
  let this = RefCell::new(Canvas::new());
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
  let width = float_arg(&mut cx, 1, "size")?;
  this.borrow_mut().width = width;
  Ok(cx.undefined())
}

pub fn set_height(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedCanvas>(0)?;
  let height = float_arg(&mut cx, 1, "size")?;
  this.borrow_mut().height = height;
  Ok(cx.undefined())
}

pub fn get_async(mut cx: FunctionContext) -> JsResult<JsBoolean> {
  let this = cx.argument::<BoxedCanvas>(0)?;
  let this = this.borrow();
  Ok(cx.boolean(this.async_io))
}

pub fn set_async(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedCanvas>(0)?;
  let go_async = cx.argument::<JsBoolean>(1)?;
  this.borrow_mut().async_io = go_async.value(&mut cx);
  Ok(cx.undefined())
}

pub fn toBuffer(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  // let this = cx.argument::<BoxedCanvas>(0)?;
  let callback = cx.argument::<JsFunction>(1)?.root(&mut cx);
  let pages = pages_arg(&mut cx, 2)?;
  let file_format = string_arg(&mut cx, 3, "format")?;
  let quality = float_arg(&mut cx, 4, "quality")?;
  let density = float_arg(&mut cx, 5, "density")?;
  let outline = bool_arg(&mut cx, 6, "outline")?;
  let matte = color_arg(&mut cx, 7);
  let channel = cx.channel();

  rayon::spawn(move || {
    let encoded = {
      if file_format=="pdf" && pages.len() > 1 {
        pages.as_pdf(quality, density)
      }else{
        pages.first().encoded_as(&file_format, quality, density, outline, matte)
      }
    };

    channel.send(move |mut cx| {
      let callback = callback.into_inner(&mut cx);
      let this = cx.undefined();

      let args = match encoded{
        Ok(data) => {
          let mut buffer = JsBuffer::new(&mut cx, data.len() as u32).unwrap();
          cx.borrow_mut(&mut buffer, |buf_data| {
            buf_data.as_mut_slice().copy_from_slice(&data);
          });
          vec![
            cx.string("ok").upcast::<JsValue>(),
            buffer.upcast::<JsValue>(),
          ]
        },
        Err(msg) => vec![
          cx.string("err").upcast::<JsValue>(),
          cx.string(msg).upcast::<JsValue>(),
        ]
      };

      callback.call(&mut cx, this, args)?;
      Ok(())
    });
  });

  Ok(cx.undefined())
}

pub fn toBufferSync(mut cx: FunctionContext) -> JsResult<JsValue> {
  // let this = cx.argument::<BoxedCanvas>(0)?;
  let pages = pages_arg(&mut cx, 1)?;
  let file_format = string_arg(&mut cx, 2, "format")?;
  let quality = float_arg(&mut cx, 3, "quality")?;
  let density = float_arg(&mut cx, 4, "density")?;
  let outline = bool_arg(&mut cx, 5, "outline")?;
  let matte = color_arg(&mut cx, 6);

    let encoded = {
      if file_format=="pdf" && pages.len() > 1 {
        pages.as_pdf(quality, density)
      }else{
        pages.first().encoded_as(&file_format, quality, density, outline, matte)
      }
    };

    match encoded{
      Ok(data) => {
        let mut buffer = JsBuffer::new(&mut cx, data.len() as u32).unwrap();
        cx.borrow_mut(&mut buffer, |buf_data| {
          buf_data.as_mut_slice().copy_from_slice(&data);
        });
        Ok(buffer.upcast::<JsValue>())
      },
      Err(msg) => cx.throw_error(msg)
    }
}


pub fn save(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  // let this = cx.argument::<BoxedCanvas>(0)?;
  let callback = cx.argument::<JsFunction>(1)?.root(&mut cx);
  let pages = pages_arg(&mut cx, 2)?;
  let name_pattern = string_arg(&mut cx, 3, "filePath")?;
  let sequence = !cx.argument::<JsValue>(4)?.is_a::<JsUndefined, _>(&mut cx);
  let padding = opt_float_arg(&mut cx, 4).unwrap_or(-1.0);
  let file_format = string_arg(&mut cx, 5, "format")?;
  let quality = float_arg(&mut cx, 6, "quality")?;
  let density = float_arg(&mut cx, 7, "density")?;
  let outline = bool_arg(&mut cx, 8, "outline")?;
  let matte = color_arg(&mut cx, 9);
  let channel = cx.channel();

  rayon::spawn(move || {
    let result = {
      if sequence {
        pages.write_sequence(&name_pattern, &file_format, padding, quality, density, outline, matte)
      } else if file_format == "pdf" {
        pages.write_pdf(&name_pattern, quality, density)
      } else {
        pages.first().write(&name_pattern, &file_format, quality, density, outline, matte)
      }
    };

    channel.send(move |mut cx| {
      let callback = callback.into_inner(&mut cx);
      let this = cx.undefined();
      let args = match result {
        Ok(_) => vec![
          cx.string("ok").upcast::<JsValue>(),
          cx.undefined().upcast::<JsValue>(),
        ],
        Err(msg) => vec![
          cx.string("err").upcast::<JsValue>(),
          cx.string(msg).upcast::<JsValue>(),
        ]
      };

      callback.call(&mut cx, this, args)?;
      Ok(())
    });
  });

  Ok(cx.undefined())
}

pub fn saveSync(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  // let this = cx.argument::<BoxedCanvas>(0)?;
  let pages = pages_arg(&mut cx, 1)?;
  let name_pattern = string_arg(&mut cx, 2, "filePath")?;
  let sequence = !cx.argument::<JsValue>(3)?.is_a::<JsUndefined, _>(&mut cx);
  let padding = opt_float_arg(&mut cx, 3).unwrap_or(-1.0);
  let file_format = string_arg(&mut cx, 4, "format")?;
  let quality = float_arg(&mut cx, 5, "quality")?;
  let density = float_arg(&mut cx, 6, "density")?;
  let outline = bool_arg(&mut cx, 7, "outline")?;
  let matte = color_arg(&mut cx, 8);

  let result = {
    if sequence {
      pages.write_sequence(&name_pattern, &file_format, padding, quality, density, outline, matte)
    } else if file_format == "pdf" {
      pages.write_pdf(&name_pattern, quality, density)
    } else {
      pages.first().write(&name_pattern, &file_format, quality, density, outline, matte)
    }
  };

  match result{
    Ok(_) => Ok(cx.undefined()),
    Err(msg) => cx.throw_error(msg)
  }
}
