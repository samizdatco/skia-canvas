#![allow(unused_mut)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]
use std::cell::RefCell;
use neon::{prelude::*, types::buffer::TypedArray};
use skia_safe::{
  Image as SkImage, ImageInfo, ISize, ColorType, AlphaType, Data, Size,
  FontMgr, Picture, PictureRecorder, Rect, image::images, svg,
  wrapper::PointerWrapper // for SVG Dom access, temporary until next skia-safe update
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
  Loading
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

  pub fn size(&self) -> Size {
    match &self {
      Content::Bitmap(img) => img.dimensions().into(),
      Content::Vector(pict) => pict.cull_rect().size().to_ceil().into(), // really cull_rect?
      _ => Size::new_empty()
    }
  }

  pub fn is_drawable(&self) -> bool {
    match &self{
      Content::Loading => false,
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

  // First try decoding the data as a bitmap
  // If it's not recognized, try parsing as SVG and create a picture if it is valid
  if let Some(image) = images::deferred_from_encoded_data(&data, None){
    this.content = Content::Bitmap(image);
  }else if let Ok(mut dom) = svg::Dom::from_bytes(&data, FONT_LIBRARY.lock().unwrap().font_mgr()){
    // Get the intrinsic size of the `svg` root element as specified in the width/height attributes, if any.
    // So far skia-safe doesn't provide direct access to the needed methods, so we have to go direct to the source.
    let i_size = unsafe { *dom.inner().containerSize() };  // skia_bindings::SkSize
    // let i_size = dom.inner().fContainerSize;  // "safe" but this is using a private member of the C++ class (somehow... skia-"safe" :-P )
    // TODO: Switch to these once available in skia-safe 0.79+
    // let mut root = dom.root();
    // let i_size = root.intrinsic_size();

    // Set a flag to indicate that the image doesn't have its own intrinsic size.
    // This may be used at drawing time if user doesn't specify a size in `drawImage()`,
    // in which case the the canvas' size will be used as the image size.
    // This is a "complication" to match Chrome's behavior... one could argue that it should
    // just be drawn at the default size (set below). Which is what FF does (though that has its own anomalies).
    let mut bounds = Rect::from_wh(i_size.fWidth, i_size.fHeight);
    this.autosized = bounds.is_empty();

    // Check if width/height are valid attribute values in the root `<svg>` element.
    // If w/h aren't specified in an SVG (which is not uncommon), both Chrome and FF will:
    //  - If only one dimension is missing then use the same size for both;
    //  - If both are missing then assign a default of 150 (which seems arbitrary but I guess as good as any);
    // `Dom::containerSize()` will return zero for both width and height if _either_ attribute is missing from `<svg>`.
    // This seems a bit suspicious (as in may change in future?), so in the interest of paranoia let's check them individually.
    // TODO: See if we can get actual width/height attribute values from DOM with skia-safe 0.79+
    (bounds.right, bounds.bottom) = match (bounds.width(), bounds.height()){
      (0.0, 0.0) => (150.0, 150.0),
      (width, 0.0) => (width, width),
      (0.0, height) => (height, height),
      (width, height) => (width, height)
    };
    dom.set_container_size(bounds.size());

    // Save the image as a Picture so it can be scaled properly later.
    let mut compositor = PictureRecorder::new();
    compositor.begin_recording(bounds, None);
    if let Some(canvas) = compositor.recording_canvas() {
      dom.render(canvas);
    }

    if let Some(picture) = compositor.finish_recording_as_picture(Some(&bounds)){
      this.content = Content::Vector(picture);
    }
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
  Ok(cx.boolean(this.content.is_drawable()))
}
