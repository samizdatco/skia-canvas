#![allow(unused_imports)]
use std::cell::RefCell;
use std::borrow::Cow;
use neon::{prelude::*, types::buffer::TypedArray};
use skia_safe::{
  Image as SkImage, ImageInfo, ISize, ColorType, ColorSpace, AlphaType, Data, Size,
  FontMgr, Matrix, Picture, PictureRecorder, Rect, image::images, matrix::ScaleToFit,
  svg::{self, Length, LengthUnit},
};
use crate::bridge::*;
use crate::context::Context2D;
use crate::font_library::FontLibrary;
use crate::gfx::cache::Cache;
use crate::gfx::page::Page;
use crate::mem;

pub type BoxedImage = JsBox<RefCell<Image>>;
impl Finalize for Image {}

pub struct Image{
  src:String,
  pub autosized:bool,
  pub content: Content,
  footprint: mem::v8::Footprint,
}

impl Default for Image{
  fn default() -> Self {
    Image{ content:Content::Loading, autosized:false, src:"".to_string(), footprint:mem::v8::Footprint::default() }
  }
}

impl Drop for Image{
  fn drop(&mut self) {
    self.content.release();
  }
}

pub enum Content{
  Bitmap(SkImage), // embeds its own intrinsic size and colorspace
  Vector(Picture, Size, ColorSpace), // needs to record them separately
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
        Content::Vector(pict, size, space) => Content::Vector(pict.clone(), size.clone(), space.clone()),
        _ => Content::default()
      }
  }
}

impl Content{
  // snapshot the context as a bitmap, already clamped to its own gamut
  pub fn raster_from_context(ctx:&mut Context2D) -> Self{
    ctx.get_image().map(|i| Content::Bitmap(i)).unwrap_or_default()
  }

  // flatten the context into a Picture, tagged with the gamut its colors were authored in
  pub fn vector_from_context(ctx:&mut Context2D) -> Self{
    let (size, space) = (ctx.bounds.size(), ctx.color_space());
    ctx.get_picture().map(|p| Content::Vector(p, size, space)).unwrap_or_default()
  }

  pub fn from_image_data(image_data:ImageData) -> Self{
    let info = image_data.image_info();
    images::raster_from_data(&info, &image_data.buffer, info.min_row_bytes())
      .map(|image| Content::Bitmap(image) )
      .unwrap_or_default()
  }

  // drop the current content safely/immediately
  pub fn release(&mut self){
    let content = std::mem::take(self);

    if let Content::Vector(pict, ..) = &content{
      // clear any cached rasters keyed to the Picture id
      let id = Page::picture_id(pict);
      Cache::shared().evict(id);
      crate::gfx::render_soon(move |cache| cache.evict(id));
    }

    if let Content::Bitmap(img) = &content{
      // drop gpu-backed bitmaps on the render_thread
      if img.is_texture_backed(){
        crate::gfx::render_soon(move |_| drop(content));
      }
    }
  }

  // Swap in new content, releasing whatever was there before.
  pub fn replace(&mut self, next: Content){
    self.release();
    *self = next;
  }

  pub fn size(&self) -> Size {
    match &self {
      Content::Bitmap(img) => img.dimensions().into(),
      Content::Vector(_, size, _) => *size,
      _ => Size::new_empty()
    }
  }

  pub fn native_bytes(&self) -> usize {
    match &self {
      // a lazily-decoded bitmap's *decoded* pixels live in Skia's resource cache (so no
      // accounting is necessary), but the *encoded* bytes are included in the v8 footprint
      Content::Bitmap(img) if img.is_lazy_generated() =>
        img.encoded_data().map(|data| data.size()).unwrap_or(0),
      Content::Bitmap(img) => img.image_info().compute_min_byte_size(),
      Content::Vector(pict, ..) => pict.approximate_bytes_used(),
      _ => 0,
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

  // shrink the src crop to just its overlap with the actual bounds and adjust dst to match
  // (or return None if there's nothing to be drawn)
  pub fn drawable_rects(size: Size, src: Rect, dst: Rect) -> Option<(Rect, Rect)> {
    // calculate the scale-factor and position shift from the src crop to dst rect (or None if src is empty)
    let placement = Matrix::rect_2_rect(src, dst, ScaleToFit::Fill)?;

    // find the actual overlap (if any) between the src crop rect and the bounds
    let mut crop = src;
    crop.intersect(Rect::from_size(size)).then(||
      (crop, placement.map_rect(crop).0) // return the clipped src and scaled/shifted dst rect
    )
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
// SVG <style> tag handling
//
// Skia's SVG DOM ignores <style> elements and CSS selectors (it honors only presentation
// attributes and inline style="…"), so we resolve the cascade ourselves and splice the winners
// back in as inline styles before handing the bytes to Skia. We resolve it fully rather than
// relying on Skia since, among other things, it discards any rule that has an `!important` keyword.
//
// Limitations: var() only uses 2-level scoping when looking up custom properties: it only looks in
// `:root` and in the element's own `style` attr (not through parent <g> groups, etc.). Also, chained
// definitions (`--a: var(--b)`) aren't supported. also, shorthand vs. longhand (font vs. font-size)
// don't cascade against each other.

struct StyledSvg<'a>(Cow<'a, [u8]>);

impl<'a> StyledSvg<'a> {
  fn from_data(data: &'a Data) -> Self {
    let svg = data.as_bytes();
    if !svg.windows(6).any(|w| w == b"<style") {
      return Self(Cow::Borrowed(svg)); // bail out early if there are no <style> tags
    }
    let Ok(text) = std::str::from_utf8(svg) else { return Self(Cow::Borrowed(svg)) };
    let Ok(doc) = roxmltree::Document::parse(text) else { return Self(Cow::Borrowed(svg)) };

    // gather CSS from every <style> element (regardless of the namespace being used)
    let mut css = String::new();
    for node in doc.descendants().filter(|n| n.tag_name().name() == "style") {
      for child in node.children() {
        if let Some(t) = child.text() { css.push_str(t); css.push('\n'); }
      }
    }
    let sheet = simplecss::StyleSheet::parse(&css);
    if sheet.rules.is_empty() {
      return Self(Cow::Borrowed(svg));
    }

    // for each element, resolve the full cascade to one keyword-stripped declaration per property,
    // merging in the element's own inline style (if any)
    let mut edits: Vec<Edit> = doc.descendants()
      .filter(|n| n.is_element() && n.tag_name().name() != "style")
      .filter_map(|node| {
        let el = StyleNode(node);
        let decls = sheet.resolve_inline(&el, el.style_value().unwrap_or(""));
        (!decls.is_empty()).then(|| el.style_edit(decls))
      })
      .collect();
    if edits.is_empty() {
      return Self(Cow::Borrowed(svg));
    }

    // apply the splices back-to-front so earlier offsets stay valid
    edits.sort_by_key(|e| e.0);
    let mut out = text.to_string();
    for (start, end, repl) in edits.into_iter().rev() {
      out.replace_range(start..end, &repl);
    }
    Self(Cow::Owned(out.into_bytes()))
  }
}

// build a Skia SVG DOM from the styled bytes or return Err(LoadError) if Skia can't parse it
impl<'a> TryFrom<StyledSvg<'a>> for svg::Dom {
  type Error = svg::LoadError;
  fn try_from(styled: StyledSvg<'a>) -> Result<Self, Self::Error> {
    svg::Dom::from_bytes(&styled.0, FontLibrary::with_shared(|lib| lib.font_mgr()))
  }
}


#[derive(Clone, Copy)]
struct StyleNode<'a, 'input>(roxmltree::Node<'a, 'input>); // a simplecss-compatible roxml node
type Edit = (usize, usize, String); // a single splice operation (start, end, replacement)

impl<'a, 'input> StyleNode<'a, 'input> {
  // the element's `style="…"` attribute, if it has one
  fn style_attr(&self) -> Option<roxmltree::Attribute<'a, 'input>> {
    self.0.attributes().find(|a| a.name() == "style")
  }

  // the element's existing inline style value, if it has one
  fn style_value(&self) -> Option<&str> {
    self.style_attr().map(|a| a.value())
  }

  // splice `decls` into an inline `style="…"` attribute, either the existing `style_attr` or a new one
  // placed after the node's last existing attribute. Coming last means it can override any presentation
  // attributes (fill="", stroke="", etc.) from earlier in the sequence.
  fn style_edit(&self, decls: String) -> Edit {
    match self.style_attr() {
      Some(style_attr) => {
        let r = style_attr.range_value();
        (r.start, r.end, decls)
      }
      None => {
        let at = self.0.attributes().last()
          .map(|a| a.range().end)
          .unwrap_or_else(|| self.tag_name_end());
        (at, at, format!(" style=\"{}\"", decls))
      }
    }
  }

  // byte offset just past `<tagname` in this element's start tag
  fn tag_name_end(&self) -> usize {
    let bytes = self.0.document().input_text().as_bytes();
    let mut i = self.0.range().start + 1; // skip '<'
    while i < bytes.len() && !matches!(bytes[i], b' '|b'\t'|b'\n'|b'\r'|b'/'|b'>') { i += 1; }
    i
  }
}

impl simplecss::Element for StyleNode<'_, '_> {
  fn parent_element(&self) -> Option<Self> {
    self.0.parent_element().map(StyleNode)
  }
  fn prev_sibling_element(&self) -> Option<Self> {
    self.0.prev_sibling_element().map(StyleNode)
  }
  fn next_sibling_element(&self) -> Option<Self> {
    self.0.next_sibling_element().map(StyleNode)
  }
  fn has_local_name(&self, name: &str) -> bool {
    self.0.tag_name().name() == name
  }
  fn local_name(&self) -> &str {
    self.0.tag_name().name()
  }
  fn attribute_matches(&self, local_name: &str, operator: simplecss::AttributeOperator) -> bool {
    self.0.attribute(local_name).map_or(false, |v| operator.matches(v))
  }
  fn pseudo_class_matches(&self, _class: simplecss::PseudoClass) -> bool {
    false // ignore :hover/:focus/:target/etc.
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

pub fn set_data<'a>(mut cx: FunctionContext<'a>) -> NeonResult<Handle<'a, JsBoolean>> {
  let this = cx.argument::<BoxedImage>(0)?;
  let mut this = this.borrow_mut();
  let buffer = cx.argument::<JsBuffer>(1)?;
  let data = Data::new_copy(buffer.as_slice(&cx));

  let new_content = if let Some(raw_info) = opt_image_info_arg(&mut cx, 2)?{
    // First, check for an optional dims argument and interpret the buffer as raw rgba if present
    match images::raster_from_data(&raw_info, data, raw_info.min_row_bytes()){
      Some(image) => Content::Bitmap(image),
      None => Content::Broken
    }
  }else if let Some(image) = images::deferred_from_encoded_data(&data, None){
    // Next, try interpreting the data as an encoded bitmap
    Content::Bitmap(image)
  }else if let Ok(mut dom) = svg::Dom::try_from(StyledSvg::from_data(&data)){
    // Finally, try parsing as SVG (resolving any <style> CSS first, since Skia's SVG DOM ignores it)
    let root = dom.root();

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
    dom.render(compositor.begin_recording(bounds, true));
    match compositor.finish_recording_as_picture(None){
      // skia's SVG parser only emits 8-bit sRGB (as of m150), so hardcode sRGB until that changes
      Some(picture) => Content::Vector(picture, size, ColorSpace::new_srgb()),
      None => Content::Broken
    }
  }else{
    Content::Broken
  };

  this.content.replace(new_content);
  let bytes = this.content.native_bytes(); // picture-size for SVG, w×h×4 for bitmap, encoded-bytes for lazy
  this.footprint.set(bytes); // record the allocation size for v8
  let drawable = this.content.is_drawable();
  drop(this); // release the RefCell borrow before calling into cx (since it might reenter)
  Ok(cx.boolean(drawable))
}

pub fn dispose(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  // Synchronously release decoded/loaded content (rather than waiting until for the next event
  // loop tick gets around to calling Drop). Using the image after disposal is caught by js
  let this = cx.argument::<BoxedImage>(0)?;
  let mut this = this.borrow_mut();
  std::mem::take(&mut this.content).release();
  this.footprint.clear(); // record the dealloc for v8
  drop(this); // release the RefCell borrow before calling into cx (since it might reenter)
  Ok(cx.undefined())
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
  let this = this.borrow_mut();
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
