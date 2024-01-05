#![allow(unused_mut)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]
use std::cell::RefCell;
use std::sync::{Arc, Mutex, MutexGuard};
use neon::prelude::*;
use skia_safe::{Canvas as SkCanvas, Surface, Paint, Path, PathOp, Image, ImageInfo, Contains,
                Matrix, Rect, Point, IPoint, Size, ISize, Color, Color4f, ColorType, Data,
                PaintStyle, BlendMode, AlphaType, ClipOp, PictureRecorder, Picture, Drawable,
                image::CachingHint, image_filters, dash_path_effect, path_1d_path_effect, path_utils};
use skia_safe::textlayout::{ParagraphStyle, TextStyle};
use skia_safe::canvas::SrcRectConstraint::Strict;
use skia_safe::path::FillType;

pub mod api;
pub mod page;

use crate::FONT_LIBRARY;
use crate::utils::*;
use crate::typography::*;
use crate::filter::{Filter, ImageFilter, FilterQuality};
use crate::gradient::{CanvasGradient, BoxedCanvasGradient};
use crate::pattern::{CanvasPattern, BoxedCanvasPattern};
use crate::texture::{CanvasTexture, BoxedCanvasTexture};
use page::{PageRecorder, Page};

const BLACK:Color = Color::BLACK;
const TRANSPARENT:Color = Color::TRANSPARENT;

pub type BoxedContext2D = JsBox<RefCell<Context2D>>;
impl Finalize for Context2D {}
unsafe impl Send for Context2D {
  // PictureRecorder is non-threadsafe
}

pub struct Context2D{
  pub bounds: Rect,
  recorder: Arc<Mutex<PageRecorder>>,
  state: State,
  stack: Vec<State>,
  path: Path,
}

#[derive(Clone)]
pub struct State{
  clip: Option<Path>,
  matrix: Matrix,
  paint: Paint,

  fill_style: Dye,
  stroke_style: Dye,
  shadow_blur: f32,
  shadow_color: Color,
  shadow_offset: Point,

  stroke_width: f32,
  line_dash_offset: f32,
  line_dash_list: Vec<f32>,
  line_dash_marker: Option<Path>,
  line_dash_fit: path_1d_path_effect::Style,

  global_alpha: f32,
  global_composite_operation: BlendMode,
  image_filter: ImageFilter,
  filter: Filter,

  font: String,
  font_variant: String,
  font_features: Vec<String>,
  char_style: TextStyle,
  graf_style: ParagraphStyle,
  text_baseline: Baseline,
  text_tracking: i32,
  text_wrap: bool,
}

impl Default for State {
  fn default() -> Self {
    let mut paint = Paint::default();
    paint
      .set_stroke_miter(10.0)
      .set_color(BLACK)
      .set_anti_alias(true)
      .set_stroke_width(1.0)
      .set_style(PaintStyle::Fill);

    let graf_style = ParagraphStyle::new();
    let mut char_style = TextStyle::new();
    char_style.set_font_size(10.0);

    State {
      clip: None,
      matrix: Matrix::new_identity(),

      paint,
      stroke_style: Dye::Color(BLACK),
      fill_style: Dye::Color(BLACK),
      stroke_width: 1.0,
      line_dash_offset: 0.0,
      line_dash_list: vec![],
      line_dash_marker: None,
      line_dash_fit: path_1d_path_effect::Style::Rotate,

      global_alpha: 1.0,
      global_composite_operation: BlendMode::SrcOver,
      image_filter: ImageFilter{ smoothing:true, quality:FilterQuality::Low },
      filter: Filter::default(),

      shadow_blur: 0.0,
      shadow_color: TRANSPARENT,
      shadow_offset: (0.0, 0.0).into(),

      font: "10px sans-serif".to_string(),
      font_variant: "normal".to_string(),
      font_features:vec![],
      char_style,
      graf_style,
      text_baseline: Baseline::Alphabetic,
      text_tracking: 0,
      text_wrap: false
    }
  }
}

impl State{
  pub fn typography(&self) -> (TextStyle, ParagraphStyle, Baseline, bool) {
    (
      self.char_style.clone(),
      self.graf_style.clone(),
      self.text_baseline,
      self.text_wrap
    )
  }

  fn dye(&self, style:PaintStyle) -> &Dye{
    if style == PaintStyle::Stroke{ &self.stroke_style }
    else{ &self.fill_style }
  }

  fn texture(&self, style:PaintStyle) -> Option<&CanvasTexture>{
    match self.dye(style) {
      Dye::Texture(texture) => Some(texture),
      _ => None
    }
  }

}

impl Context2D{
  pub fn new() -> Self {
    let bounds = Rect::from_wh(300.0, 150.0);

    Context2D{
      bounds,
      recorder: Arc::new(Mutex::new(PageRecorder::new(bounds))),
      path: Path::new(),
      stack: vec![],
      state: State::default(),
    }
  }

  pub fn in_local_coordinates(&mut self, x: f32, y: f32) -> Point{
    match self.state.matrix.invert(){
      Some(inverse) => inverse.map_point((x, y)),
      None => (x, y).into()
    }
  }

  pub fn width(&self) -> f32{
    self.bounds.width()
  }

  pub fn height(&self) -> f32{
    self.bounds.height()
  }

  pub fn with_recorder<F>(&self, f:F)
    where F:FnOnce(MutexGuard<PageRecorder>)
  {
    let recorder = Arc::clone(&self.recorder);
    let recorder = recorder.lock().unwrap();
    f(recorder);
  }

  pub fn with_canvas<F>(&self, f:F)
    where F:FnOnce(&SkCanvas)
  {
    self.with_recorder(|mut recorder|{
      recorder.append(f);
    });
  }

  pub fn with_matrix<F>(&mut self, f:F)
    where F:FnOnce(&mut Matrix) -> &Matrix
  {
    f(&mut self.state.matrix);
    self.with_recorder(|mut recorder|{
      recorder.set_matrix(self.state.matrix);
    });
  }

  pub fn render_to_canvas<F>(&self, paint:&Paint, f:F)
    where F:Fn(&SkCanvas, &Paint)
  {
    match self.state.global_composite_operation{
      BlendMode::SrcIn | BlendMode::SrcOut |
      BlendMode::DstIn | BlendMode::DstOut |
      BlendMode::DstATop | BlendMode::Src =>{
        // for blend modes that affect regions of the canvas outside of the bounds of the object
        // being drawn, create an intermediate picture before drawing to the canvas
        let mut layer_paint = paint.clone();
        layer_paint.set_blend_mode(BlendMode::SrcOver);
        let mut layer_recorder = PictureRecorder::new();
        layer_recorder.begin_recording(self.bounds, None);
        if let Some(layer) = layer_recorder.recording_canvas() {
          // draw the dropshadow (if applicable)
          if let Some(shadow_paint) = self.paint_for_shadow(&layer_paint){
            layer.save();
            layer.set_matrix(&Matrix::translate(self.state.shadow_offset).into());
            layer.concat(&self.state.matrix);
            f(layer, &shadow_paint);
            layer.restore();
          }

          // draw normally
          layer.set_matrix(&self.state.matrix.into());
          f(layer, &layer_paint);
        }


        // transfer the picture contents to the canvas in a single operation, applying the blend
        // mode to the whole canvas (regardless of the bounds of the text/path being drawn)
        if let Some(pict) = layer_recorder.finish_recording_as_picture(Some(&self.bounds)){
          self.with_canvas(|canvas| {
            canvas.save();
            canvas.set_matrix(&Matrix::new_identity().into());
            let mut blend_paint = Paint::default();
            blend_paint.set_anti_alias(true);
            blend_paint.set_blend_mode(self.state.global_composite_operation);
            canvas.draw_picture(&pict, None, Some(&blend_paint));
            canvas.restore();
          });
        }

      },
      _ => {
        self.with_canvas(|canvas| {
          if let Some(shadow_paint) = self.paint_for_shadow(paint){
            canvas.save();
            canvas.set_matrix(&Matrix::translate(self.state.shadow_offset).into());
            canvas.concat(&self.state.matrix);
            f(canvas, &shadow_paint);
            canvas.restore();
          }

          // draw with the normal paint
          f(canvas, paint);
        });
      }
    };

  }

  pub fn map_points(&self, coords:&[f32]) -> Vec<Point>{
    coords.chunks_exact(2)
          .map(|pair| self.state.matrix.map_xy(pair[0], pair[1]))
          .collect()
  }

  pub fn reset_size(&mut self, dims: impl Into<Size>) {
    // called by the canvas when .width or .height are assigned to
    self.bounds = Rect::from_size(dims);
    self.path = Path::default();
    self.stack = vec![];
    self.state = State::default();

    // erase any existing content
    self.with_recorder(|mut recorder| {
      recorder.set_bounds(self.bounds);
    });
  }

  pub fn resize(&mut self, dims: impl Into<Size>) {
    // non-destructively resize the canvas (via the canvas.resize() extension)
    self.bounds = Rect::from_size(dims);
    self.with_recorder(|mut recorder| {
      recorder.update_bounds(self.bounds);
    });
  }

  pub fn push(&mut self){
    let new_state = self.state.clone();
    self.stack.push(new_state);
  }

  pub fn pop(&mut self){
    // don't do anything if we're already back at the initial stack frame
    if let Some(old_state) = self.stack.pop(){
      self.state = old_state;

      self.with_recorder(|mut recorder|{
        recorder.set_matrix(self.state.matrix);
        recorder.set_clip(&self.state.clip);
      });
    }
  }

  pub fn draw_path(&mut self, path:Option<Path>, style:PaintStyle, rule:Option<FillType>){
    let mut path = path.unwrap_or_else(|| {
      // the current path has already incorporated its transform state
      let inverse = self.state.matrix.invert().unwrap();
      self.path.with_transform(&inverse)
    });
    path.set_fill_type(rule.unwrap_or(FillType::Winding));

    let paint = self.paint_for_drawing(style);
    let texture = self.state.texture(style);

    self.render_to_canvas(&paint, |canvas, paint| {
      if let Some(tile) = texture{
        canvas.save();
        let spacing = tile.spacing();
        let offset = (-spacing.0/2.0, -spacing.1/2.0);

        let mut stencil = Path::default();
        path_utils::fill_path_with_paint(&path, &paint, &mut stencil, None, None);
        let stencil_frame = &Path::rect(stencil.bounds().with_offset(offset).with_outset(spacing), None);

        let mut tile_paint = paint.clone();
        tile.mix_into(&mut tile_paint, self.state.global_alpha);

        let mut tile_path = Path::default();
        path_utils::fill_path_with_paint(&stencil_frame, &tile_paint, &mut tile_path, None, None);

        let mut fill_paint = paint.clone();
        fill_paint.set_style(PaintStyle::Fill);
        if let Some(fill_path) = stencil.op(&tile_path, PathOp::Intersect){
          canvas.draw_path(&fill_path, &fill_paint);
        }
      }else{
        canvas.draw_path(&path, paint);
      }
    });
  }

  pub fn clip_path(&mut self, path: Option<Path>, rule:FillType){
    let mut clip = path.unwrap_or_else(|| self.path.clone()) ;
    clip.set_fill_type(rule);

    self.state.clip = match &self.state.clip {
      Some(old_clip) => old_clip.op(&clip, PathOp::Intersect),
      None => Some(clip.clone())
    };

    self.with_recorder(|mut recorder|{
      recorder.set_clip(&self.state.clip);
    });
  }

  pub fn hit_test_path(&mut self, path: &mut Path, point:impl Into<Point>, rule:Option<FillType>, style: PaintStyle) -> bool {
    let point = point.into();
    let point = self.in_local_coordinates(point.x, point.y);
    let rule = rule.unwrap_or(FillType::Winding);
    let prev_rule = path.fill_type();
    path.set_fill_type(rule);

    let is_in = match style{
      PaintStyle::Stroke => {
        let paint = self.paint_for_drawing(PaintStyle::Stroke);
        let precision = 0.3; // this is what Chrome uses to compute this
        let matrix = Matrix::scale((precision, precision));
        let mut traced_path = Path::default();
        match path_utils::fill_path_with_paint(path, &paint, &mut traced_path, None, Some(matrix)){
          true => traced_path.contains(point),
          false => path.contains(point)
        }
      },
      _ => path.contains(point)
    };

    path.set_fill_type(prev_rule);
    is_in
  }

  pub fn clear_rect(&mut self, rect:&Rect){
    match self.state.matrix.map_rect(rect).0.contains(self.bounds){

      // if rect fully encloses canvas, erase existing content (but preserve CTM, path, etc.)
      true =>  self.with_recorder(|mut recorder|{
        recorder.set_bounds(self.bounds);
        recorder.set_matrix(self.state.matrix);
        recorder.set_clip(&self.state.clip);
      }),

      // otherwise, paint over the specified region but preserve overdrawn vectors
      false => self.with_canvas(|canvas| {
        let mut paint = Paint::default();
        paint.set_anti_alias(true)
             .set_style(PaintStyle::Fill)
             .set_blend_mode(BlendMode::Clear);
        canvas.draw_rect(&rect, &paint);
      })
    }
  }

  pub fn draw_picture(&mut self, picture:&Option<Picture>, src_rect:&Rect, dst_rect:&Rect){
    let paint = self.paint_for_image();
    let size = ISize::new(dst_rect.width() as i32, dst_rect.height() as i32);
    let mag = Point::new(dst_rect.width()/src_rect.width(), dst_rect.height()/src_rect.height());
    let mut matrix = Matrix::new_identity();
    matrix.pre_scale( (mag.x, mag.y), None )
    .pre_translate((dst_rect.x()/mag.x - src_rect.x(), dst_rect.y()/mag.y - src_rect.y()));

    if let Some(picture) = picture{
      self.render_to_canvas(&paint, |canvas, paint| {
        // only use paint if we need it for alpha, blend, shadow, or effect since otherwise
        // the SVG exporter will omit the picture altogether
        let paint = match (paint.as_blend_mode(), paint.alpha(), paint.image_filter()) {
          (Some(BlendMode::SrcOver), 255, None) => None,
          _ => Some(paint)
        };
        canvas.draw_picture(&picture, Some(&matrix), paint);
      });
    }
  }

  pub fn draw_image(&mut self, img:&Option<Image>, src_rect:&Rect, dst_rect:&Rect){
    let paint = self.paint_for_image();
    if let Some(image) = &img {
      self.render_to_canvas(&paint, |canvas, paint| {
        let sampling = self.state.image_filter.sampling();
        canvas.draw_image_rect_with_sampling_options(&image, Some((src_rect, Strict)), dst_rect, sampling, paint);
      });
    }
  }

  pub fn get_page(&self) -> Page {
    let recorder = Arc::clone(&self.recorder);
    let mut recorder = recorder.lock().unwrap();
    recorder.get_page()
  }

  pub fn get_image(&self) -> Option<Image> {
    let recorder = Arc::clone(&self.recorder);
    let mut recorder = recorder.lock().unwrap();
    recorder.get_image()
  }

  pub fn get_picture(&mut self) -> Option<Picture> {
    self.get_page().get_picture(None)
  }

  pub fn get_pixels(&mut self, buffer: &mut [u8], origin: impl Into<IPoint>, size: impl Into<ISize>){
    let origin = origin.into();
    let size = size.into();
    let info = ImageInfo::new(size, ColorType::RGBA8888, AlphaType::Unpremul, None);

    if let Some(img) = self.get_image(){
      img.read_pixels(&info, buffer, info.min_row_bytes(), origin, CachingHint::Allow);
    }
  }

  pub fn blit_pixels(&mut self, buffer: &[u8], info: &ImageInfo, src_rect:&Rect, dst_rect:&Rect){
    // works just like draw_image in terms of src/dst rects, but clears the dst_rect and then draws
    // without clips, transforms, alpha, blend, or shadows
    let data = Data::new_copy(buffer);
    if let Some(bitmap) = Image::from_raster_data(info, data, info.min_row_bytes()) {
      self.push(); // cache matrix & clip in self.state
      self.with_canvas(|canvas| {
        let paint = Paint::default();
        let mut eraser = Paint::default();
        canvas.restore_to_count(1); // discard current matrix & clip
        eraser.set_blend_mode(BlendMode::Clear);
        canvas.draw_image_rect(&bitmap, Some((src_rect, Strict)), dst_rect, &eraser);
        canvas.draw_image_rect(&bitmap, Some((src_rect, Strict)), dst_rect, &paint);
      });
      self.pop(); // restore discarded matrix & clip
    }
  }

  pub fn set_font(&mut self, spec: FontSpec){
    let mut library = FONT_LIBRARY.lock().unwrap();
    if let Some(new_style) = library.update_style(&self.state.char_style, &spec){
      self.state.font = spec.canonical;
      self.state.font_variant = spec.variant.to_string();
      self.state.char_style = new_style;
    }
  }

  pub fn set_font_variant(&mut self, variant:&str, features:&[(String, i32)]){
    let mut library = FONT_LIBRARY.lock().unwrap();
    let new_style = library.update_features(&self.state.char_style, features);
    self.state.font_variant = variant.to_string();
    self.state.char_style = new_style;
  }


  pub fn draw_text(&mut self, text: &str, x: f32, y: f32, width: Option<f32>, style:PaintStyle){
    let paint = self.paint_for_drawing(style);
    let typesetter = Typesetter::new(&self.state, text, width);
    self.render_to_canvas(&paint, |canvas, paint| {
      let point = Point::new(x, y);
      let (paragraph, offset) = typesetter.layout(paint);
      paragraph.paint(canvas, point + offset);
    });
  }

  pub fn measure_text(&mut self, text: &str, width:Option<f32>) -> Vec<Vec<f32>>{
    Typesetter::new(&self.state, text, width).metrics()
  }

  pub fn outline_text(&self, text:&str) -> Option<Path>{
    Typesetter::new(&self.state, text, None).path()
  }

  pub fn color_with_alpha(&self, src:&Color) -> Color{
    let mut color:Color4f = (*src).into();
    color.a *= self.state.global_alpha;
    color.to_color()
  }

  pub fn paint_for_drawing(&mut self, style:PaintStyle) -> Paint{
    let mut paint = self.state.paint.clone();
    self.state.filter.mix_into(&mut paint, self.state.matrix, false);
    self.state.dye(style).mix_into(&mut paint, self.state.global_alpha, self.state.image_filter);
    paint.set_style(style);

    if style==PaintStyle::Stroke && !self.state.line_dash_list.is_empty(){
      // if marker is set, apply the 1d_path_effect instead of the dash_path_effect

      let effect = match &self.state.line_dash_marker{
        Some(path) => {
          let marker = match path.is_last_contour_closed(){
            true => path.clone(),
            false => {
              let mut fill_path = Path::default();
              path_utils::fill_path_with_paint(&path, &paint, &mut fill_path, None, None);
              fill_path
            }
          };
          path_1d_path_effect::new(
            &marker,
            self.state.line_dash_list[0],
            self.state.line_dash_offset,
            self.state.line_dash_fit
          )
        }
        None => dash_path_effect::new(&self.state.line_dash_list, self.state.line_dash_offset)
      };

      paint.set_path_effect(effect);
    }

    paint
  }

  pub fn paint_for_image(&mut self) -> Paint {
    let mut paint = self.state.paint.clone();
    self.state.filter.mix_into(&mut paint, self.state.matrix, true)
      .set_alpha_f(self.state.global_alpha);
    paint
  }

  pub fn paint_for_shadow(&self, base_paint:&Paint) -> Option<Paint> {
    let State {shadow_color, shadow_blur, shadow_offset, ..} = self.state;
    if shadow_color.a() == 0 || (shadow_blur == 0.0 && shadow_offset.is_zero()){
      return None
    }

    let sigma_x = shadow_blur / (2.0 * self.state.matrix.scale_x());
    let sigma_y = shadow_blur / (2.0 * self.state.matrix.scale_y());
    let mut paint = base_paint.clone();
    paint.set_image_filter(image_filters::drop_shadow_only((0.0, 0.0), (sigma_x, sigma_y), shadow_color, None, None));
    Some(paint)
  }

}

//
// Dye abstraction for Color / CanvasGradient / CanvasPattern
//

#[derive(Clone)]
pub enum Dye{
  Color(Color),
  Gradient(CanvasGradient),
  Pattern(CanvasPattern),
  Texture(CanvasTexture)
}

impl Dye{
  pub fn new<'a>(cx: &mut FunctionContext<'a>, value: Handle<'a, JsValue>) -> Option<Self> {
    if let Ok(gradient) = value.downcast::<BoxedCanvasGradient, _>(cx){
      Some(Dye::Gradient(gradient.borrow().clone()) )
    }else if let Ok(pattern) = value.downcast::<BoxedCanvasPattern, _>(cx){
      Some(Dye::Pattern(pattern.borrow().clone()) )
    }else if let Ok(texture) = value.downcast::<BoxedCanvasTexture, _>(cx){
      Some(Dye::Texture(texture.borrow().clone()) )
    }else{
      color_in(cx, value).map(Dye::Color)
    }
  }

  pub fn value<'a>(&self, cx: &mut FunctionContext<'a>) -> JsResult<'a, JsValue> {
    match self{
      Dye::Color(color) => color_to_css(cx, color),
      _ => Ok(cx.null().upcast()) // flag to the js context that it should use its cached pattern/gradient ref
    }
  }

  pub fn mix_into(&self, paint: &mut Paint, alpha: f32, image_filter: ImageFilter){
    match self {
      Dye::Color(color) => {
        let mut color:Color4f = (*color).into();
        color.a *= alpha;
        paint.set_color(color.to_color());
      },
      Dye::Gradient(gradient) =>{
        paint.set_shader(gradient.shader())
             .set_alpha_f(alpha);
      },
      Dye::Pattern(pattern) =>{
        paint.set_shader(pattern.shader(image_filter))
             .set_alpha_f(alpha);
      }
      Dye::Texture(texture) =>{
        paint.set_color(texture.to_color(alpha));
      }
    };
  }
}
