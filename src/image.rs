#![allow(unused_mut)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]
use std::cell::RefCell;
use neon::{prelude::*, types::buffer::TypedArray};
use skia_safe::{
  Image as SkImage, ImageInfo, ISize, ColorType, ColorSpace, AlphaType, Data, Size,
  FontMgr, Picture, PictureRecorder, Rect, image::images,
  svg::{self, Length, LengthUnit},
  // wrapper::PointerWrapper // for SVG Dom access, temporary until next skia-safe update
};
use crate::utils::*;
use crate::context::Context2D;
use crate::FONT_LIBRARY;

pub type BoxedImage = JsBox<RefCell<Image>>;
impl Finalize for Image {}

pub struct Image{
  src:String,
  pub autosized:bool,
  pub content: Content,
}

impl Default for Image{
  fn default() -> Self {
    Image{ content:Content::Loading, autosized:false, src:"".to_string() }
  }
}

pub enum Content{
  Bitmap(SkImage),
  Vector(Picture),
  Loading,
  Broken,
}

impl Default for Content{
  fn default() -> Self {
      Content::Loading
  }
}

impl Clone for Content{
  fn clone(&self) -> Self {
      match self{
        Content::Bitmap(img) => Content::Bitmap(img.clone()),
        Content::Vector(pict) => Content::Vector(pict.clone()),
        _ => Content::default()
      }
  }
}

impl Content{
  pub fn from_context(ctx:&mut Context2D, use_vector:bool) -> Self{
    match use_vector{
      true => ctx.get_picture().map(|p| Content::Vector(p)),
      false => ctx.get_image().map(|i| Content::Bitmap(i)),
    }.unwrap_or_default()
  }

  pub fn from_image_data(image_data:ImageData) -> Self{
    let info = image_data.image_info();
    images::raster_from_data(&info, &image_data.buffer, info.min_row_bytes())
      .map(|image| Content::Bitmap(image) )
      .unwrap_or_default()
  }

  pub fn size(&self) -> Size {
    match &self {
      Content::Bitmap(img) => img.dimensions().into(),
      Content::Vector(pict) => pict.cull_rect().size().to_ceil().into(), // really cull_rect?
      _ => Size::new_empty()
    }
  }

  pub fn is_complete(&self) -> bool {
    match &self{
      Content::Loading => false,
      _ => true
    }
  }

  pub fn is_drawable(&self) -> bool {
    match &self{
      Content::Loading | Content::Broken => false,
      _ => true
    }
  }

  pub fn snap_rects_to_bounds(&self, mut src: Rect, mut dst: Rect) -> (Rect, Rect) {
    // Handle 'overdraw' of the src image where the crop coordinates are outside of its bounds
    // Snap the src rect to its actual bounds and shift/pad the dst rect to account for the
    // whitespace included in the crop.
    let scale_x = dst.width() / src.width();
    let scale_y = dst.height() / src.height();
    let size = self.size();

    if src.left < 0.0 {
      dst.left += -src.left * scale_x;
      src.left = 0.0;
    }

    if src.top < 0.0 {
      dst.top += -src.top * scale_y;
      src.top = 0.0;
    }

    if src.right > size.width{
      dst.right -= (src.right - size.width) * scale_x;
      src.right = size.width;
    }

    if src.bottom > size.height{
      dst.bottom -= (src.bottom - size.height) * scale_y;
      src.bottom = size.height;
    }

    (src, dst)
  }
}


#[derive(Debug)]
pub struct ImageData{
  pub width: f32,
  pub height: f32,
  pub buffer: Data,
  color_type: ColorType,
  color_space: ColorSpace,
}

impl ImageData{
  pub fn new(buffer:Data, width:f32, height:f32, color_type:String, color_space:String) -> Self{
    let color_type = to_color_type(&color_type);
    let color_space = to_color_space(&color_space);
    Self{ buffer, width, height, color_type, color_space }
  }

  pub fn image_info(&self) -> ImageInfo{
    ImageInfo::new(
      (self.width as _, self.height as _),
      self.color_type,
      AlphaType::Unpremul,
      self.color_space.clone()
    )
  }
}



//
// -- Javascript Methods --------------------------------------------------------------------------
//

pub fn new(mut cx: FunctionContext) -> JsResult<BoxedImage> {
  let this = RefCell::new(Image::default());
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
  let data = Data::new_copy(buffer.as_slice(&cx));

  // First try decoding the data as a bitmap, if invalid try parsing as SVG
  if let Some(image) = images::deferred_from_encoded_data(&data, None){
    this.content = Content::Bitmap(image);
  }else if let Ok(mut dom) = svg::Dom::from_bytes(&data, FONT_LIBRARY.lock().unwrap().font_mgr()){
    let mut root = dom.root();

    let mut size = root.intrinsic_size();
    if size.is_empty(){
      // flag that image lacks an intrinsic size so it will be drawn to match the canvas size
      // if dimensions aren't provided in the drawImage() call
      this.autosized = true;

      // If width or height attributes aren't defined on the root `<svg>` element, they will be reported as "100%".
      // If only one is defined, use it for both dimensions, and if both are missing use the aspect ratio to scale the
      // width vs a fixed height of 150 (i.e., Chrome's behavior)
      let Length{ value:width, unit:w_unit } = root.width();
      let Length{ value:height, unit:h_unit } = root.height();
      size = match ((width, w_unit), (height, h_unit)){
        // NB: only unitless numeric lengths are currently being handled; values in em, cm, in, etc. are ignored,
        // but perhaps they should be converted to px?
        ((100.0, LengthUnit::Percentage), (height, LengthUnit::Number)) => (*height, *height).into(),
        ((width, LengthUnit::Number),     (100.0,  LengthUnit::Percentage)) => (*width, *width).into(),
        _ => {
          let aspect = root.view_box().map(|vb| vb.width()/vb.height()).unwrap_or(1.0);
          (150.0 * aspect, 150.0).into()
        }
      };
    };

    // Save the SVG contents as a Picture (to be drawn later)
    let bounds = Rect::from_size(size);
    let mut compositor = PictureRecorder::new();
    dom.set_container_size(bounds.size());
    dom.render(compositor.begin_recording(bounds, None));
    if let Some(picture) = compositor.finish_recording_as_picture(Some(&bounds)){
      this.content = Content::Vector(picture);
    }
  }else{
    this.content = Content::Broken
  }

  Ok(cx.boolean(this.content.is_drawable()))
}

pub fn get_width(mut cx: FunctionContext) -> JsResult<JsValue> {
  let this = cx.argument::<BoxedImage>(0)?;
  let this = this.borrow();
  Ok(cx.number(this.content.size().width).upcast())
}

pub fn get_height(mut cx: FunctionContext) -> JsResult<JsValue> {
  let this = cx.argument::<BoxedImage>(0)?;
  let this = this.borrow();
  Ok(cx.number(this.content.size().height).upcast())
}

pub fn get_complete(mut cx: FunctionContext) -> JsResult<JsBoolean> {
  let this = cx.argument::<BoxedImage>(0)?;
  let this = this.borrow();
  Ok(cx.boolean(this.content.is_complete()))
}

pub fn pixels(mut cx: FunctionContext) -> JsResult<JsValue> {
  let this = cx.argument::<BoxedImage>(0)?;
  let mut this = this.borrow_mut();
  let (color_type, color_space) = image_data_settings_arg(&mut cx, 1);

  let info = ImageInfo::new(this.content.size().to_floor(), color_type, AlphaType::Unpremul, color_space);
  let mut pixels = cx.buffer(info.bytes_per_pixel() * (info.width() * info.height()) as usize)?;

  match &this.content{
    Content::Bitmap(image) => {
      match image.read_pixels(&info, pixels.as_mut_slice(&mut cx), info.min_row_bytes(), (0,0), skia_safe::image::CachingHint::Allow){
        true => Ok(pixels.upcast()),
        false => Ok(cx.undefined().upcast())
      }

    }
    _ => Ok(cx.undefined().upcast())
  }
}
