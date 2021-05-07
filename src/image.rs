#![allow(unused_mut)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]
use std::cell::RefCell;
use neon::prelude::*;
use skia_safe::{Image as SkImage, ImageInfo, ColorType, AlphaType, Data};

use crate::utils::*;


pub type BoxedImage = JsBox<RefCell<Image>>;
impl Finalize for Image {}

pub struct Image{
  src:String,
  pub image:Option<SkImage>
}

impl Image{
  pub fn info(width:f32, height:f32) -> ImageInfo {
    let dims = (width as i32, height as i32);
    ImageInfo::new(dims, ColorType::RGBA8888, AlphaType::Unpremul, None)
  }
}

//
// -- Javascript Methods --------------------------------------------------------------------------
//

pub fn new(mut cx: FunctionContext) -> JsResult<BoxedImage> {
  let this = RefCell::new(Image{ src:"".to_string(), image:None });
  Ok(cx.boxed(this))
}

pub fn get_src(mut cx: FunctionContext) -> JsResult<JsString> {
  let this = cx.argument::<BoxedImage>(0)?;
  let this = this.borrow();

  Ok(cx.string(&this.src))
}

pub fn set_src(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let this = cx.argument::<BoxedImage>(0)?;
  let mut this = this.borrow_mut();

  let src = cx.argument::<JsString>(1)?.value(&mut cx);
  this.src = src;
  Ok(cx.undefined())
}

pub fn set_data(mut cx: FunctionContext) -> JsResult<JsBoolean> {
  let this = cx.argument::<BoxedImage>(0)?;
  let mut this = this.borrow_mut();

  let buffer = cx.argument::<JsBuffer>(1)?;
  let data = cx.borrow(&buffer, |buf_data| {
    Data::new_copy(buf_data.as_slice())
  });

  this.image = SkImage::from_encoded(data);
  Ok(cx.boolean(this.image.is_some()))
}

pub fn get_width(mut cx: FunctionContext) -> JsResult<JsValue> {
  let this = cx.argument::<BoxedImage>(0)?;
  let this = this.borrow();

  match &this.image {
    Some(image) => Ok(cx.number(image.width() as f64).upcast()),
    None => Ok(cx.undefined().upcast())
  }
}

pub fn get_height(mut cx: FunctionContext) -> JsResult<JsValue> {
  let this = cx.argument::<BoxedImage>(0)?;
  let this = this.borrow();

  match &this.image {
    Some(image) => Ok(cx.number(image.height() as f64).upcast()),
    None => Ok(cx.undefined().upcast())
  }
}

pub fn get_complete(mut cx: FunctionContext) -> JsResult<JsBoolean> {
  let this = cx.argument::<BoxedImage>(0)?;
  let this = this.borrow();
  Ok(cx.boolean(this.image.is_some()))
}
