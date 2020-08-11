#![allow(unused_mut)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]
use neon::prelude::*;
use neon::object::This;
use neon::result::Throw;
use skia_safe::{Surface, Path, Matrix, Paint, Rect, Point, Color, Color4f, Image, PaintStyle,
                BlendMode, FilterQuality, dash_path_effect, ClipOp, image_filters, FontMgr};
use skia_safe::canvas::SrcRectConstraint;
use skia_safe::path::FillType;
use skia_safe::textlayout::{FontCollection, TextStyle, TextAlign, TextDirection, TextShadow,
                            ParagraphStyle, ParagraphBuilder, Paragraph};

use crate::utils::*;
use crate::gradient::{CanvasGradient, JsCanvasGradient};
use crate::pattern::{CanvasPattern, JsCanvasPattern};

const BLACK:Color = Color::BLACK;
const TRANSPARENT:Color = Color::TRANSPARENT;
const GALLEY:f32 = 100_000.0;

pub mod class;
pub use class::JsContext2D;

pub struct Context2D{
  pub surface: Option<Surface>,
  pub path: Path,
  pub state_stack: Vec<State>,
  pub state: State,
}

#[derive(Clone)]
pub struct State{
  pub paint: Paint,

  pub fill_style: Dye,
  pub stroke_style: Dye,
  pub shadow_blur: f32,
  pub shadow_color: Color,
  pub shadow_offset: Point,

  pub global_alpha: f32,
  pub stroke_width: f32,
  pub line_dash_offset: f32,
  pub line_dash_list: Vec<f32>,

  pub global_composite_operation: BlendMode,
  pub image_filter_quality: FilterQuality,
  pub image_smoothing_enabled: bool,

  pub font: String,
  pub font_variant: String,
  pub font_features: Vec<String>,
  pub char_style: TextStyle,
  pub graf_style: ParagraphStyle,
  pub text_baseline: Baseline,
  pub text_tracking: i32,
}

#[derive(Clone)]
pub enum Dye{
  Color(Color),
  Gradient(CanvasGradient),
  Pattern(CanvasPattern)
}

impl Dye{
  pub fn new<'a, T: This+Class>(cx: &mut CallContext<'a, T>, value: Handle<'a, JsValue>, style: PaintStyle) -> Result<Self, Throw> {
    let stash = if style == PaintStyle::Fill{ "fillShader" } else { "strokeShader" };
    match value{
      arg if arg.is_a::<JsCanvasGradient>() => {
        let gradient = cx.argument::<JsCanvasGradient>(0)?;
        stash_ref(cx, stash, arg)?;
        Ok(cx.borrow(&gradient, |gradient| Dye::Gradient(gradient.clone()) ))
      },
      arg if arg.is_a::<JsCanvasPattern>() => {
        let pattern = cx.argument::<JsCanvasPattern>(0)?;
        stash_ref(cx, stash, arg)?;
        Ok(cx.borrow(&pattern, |pattern| Dye::Pattern(pattern.clone()) ))
      },
      _ => {
        let color = color_arg(cx, 0)?;
        Ok(Dye::Color(color))
      }
    }
  }

  pub fn value<'a, T: This+Class>(&self, cx: &mut CallContext<'a, T>, style: PaintStyle) -> JsResult<'a, JsValue> {
    let cache = if style == PaintStyle::Fill{ "fillShader" } else { "strokeShader" };
    match self{
      Dye::Gradient(..) => fetch_ref(cx, cache),
      Dye::Pattern(..)  => fetch_ref(cx, cache),
      Dye::Color(color) => color_to_css(cx, &color)
    }
  }

  pub fn mix_into(&self, paint: &mut Paint, alpha: f32){
    match self {
      Dye::Color(color) => {
        let mut color:Color4f = color.clone().into();
        color.a *= alpha;
        paint.set_color(color.to_color())
      },
      Dye::Gradient(gradient) => paint.set_shader(gradient.shader()),
      Dye::Pattern(pattern) => paint.set_shader(pattern.shader())
    };
  }
}

impl Context2D{
  pub fn new() -> Self {
    let mut paint = Paint::default();
    paint.set_stroke_miter(10.0);
    paint.set_color(BLACK);
    paint.set_anti_alias(true);
    paint.set_stroke_width(1.0);
    paint.set_filter_quality(FilterQuality::Low);

    let mut char_style = TextStyle::new();
    char_style.set_font_size(10.0);

    let mut graf_style = ParagraphStyle::new();
    graf_style.set_text_align(TextAlign::Start);
    graf_style.set_text_direction(TextDirection::LTR);

    Context2D{
      surface: None,
      path: Path::new(),
      state_stack: vec![],

      state: State {
        paint,
        stroke_style: Dye::Color(BLACK),
        fill_style: Dye::Color(BLACK),

        stroke_width: 1.0,
        line_dash_offset: 0.0,
        line_dash_list: vec![],

        global_alpha: 1.0,
        global_composite_operation: BlendMode::SrcOver,
        image_filter_quality: FilterQuality::Low,
        image_smoothing_enabled: true,

        shadow_blur: 0.0,
        shadow_color: TRANSPARENT,
        shadow_offset: (0.0, 0.0).into(),

        font: "10px monospace".to_string(),
        font_variant: "normal".to_string(),
        font_features:vec![],
        char_style,
        graf_style,
        text_baseline: Baseline::Alphabetic,
        text_tracking: 0,
      },
    }
  }

  pub fn ctm(&mut self) -> Matrix {
    let mut canvas = self.surface.as_mut().unwrap().canvas();
    canvas.total_matrix()
  }

  pub fn in_local_coordinates(&mut self, x: f32, y: f32) -> Point{
    match self.ctm().invert(){
      Some(inverse) => inverse.map_point((x, y)),
      None => (x, y).into()
    }
  }

  pub fn push(&mut self){
    let mut canvas = self.surface.as_mut().unwrap().canvas();
    let new_state = self.state.clone();
    self.state_stack.push(new_state);
    canvas.save();
  }

  pub fn pop(&mut self){
    let mut canvas = self.surface.as_mut().unwrap().canvas();
    if let Some(old_state) = self.state_stack.pop(){
      self.state = old_state;
    }
    canvas.restore();
  }

  pub fn draw_path(&mut self, paint: &Paint){
    let shadow = self.paint_for_shadow(&paint);
    let mut canvas = self.surface.as_mut().unwrap().canvas();

    // draw shadow if applicable
    if let Some(shadow_paint) = shadow{
      canvas.draw_path(&self.path, &shadow_paint);
    }

    // then draw the actual path
    canvas.draw_path(&self.path, &paint);
  }

  pub fn clip_path(&mut self, path: Option<Path>, rule:FillType){
    let do_aa = true;
    let mut canvas = self.surface.as_mut().unwrap().canvas();

    let mut clip = match path{
      Some(path) => path,
      None => self.path.clone()
    };

    clip.set_fill_type(rule);
    canvas.clip_path(&clip, ClipOp::Intersect, do_aa);
  }

  pub fn hit_test_path(&mut self, path: &mut Path, point:impl Into<Point>, rule:Option<FillType>, style: PaintStyle) -> bool {
    let point = point.into();
    let point = self.in_local_coordinates(point.x, point.y);
    let rule = rule.unwrap_or(FillType::Winding);
    let prev_rule = path.fill_type();
    path.set_fill_type(rule);

    let is_in = match style{
      PaintStyle::Stroke => {
        let paint = self.paint_for_stroke();
        let precision = 0.3; // this is what Chrome uses to compute this
        match paint.get_fill_path(&path, None, Some(precision)){
          Some(traced_path) => traced_path.contains(point),
          None => path.contains(point)
        }
      },
      _ => path.contains(point)
    };

    path.set_fill_type(prev_rule);
    is_in
}

  pub fn draw_rect(&mut self, rect:&Rect, paint: &Paint){
    let shadow = self.paint_for_shadow(&paint);
    let mut canvas = self.surface.as_mut().unwrap().canvas();

    // draw shadow if applicable
    if let Some(shadow_paint) = shadow{
      canvas.draw_rect(&rect, &shadow_paint);
    }

    // then draw the actual rect
    canvas.draw_rect(&rect, &paint);
  }

  pub fn clear_rect(&mut self, rect:&Rect){
    let mut canvas = self.surface.as_mut().unwrap().canvas();
    let mut paint = Paint::default();
    paint.set_style(PaintStyle::Fill);
    paint.set_blend_mode(BlendMode::Clear);
    canvas.draw_rect(&rect, &paint);
  }

  pub fn draw_image(&mut self, img:&Option<Image>, src_rect:&Rect, dst_rect:&Rect){
    let mut paint = self.state.paint.clone();
    paint.set_style(PaintStyle::Fill);
    paint.set_color(self.color_with_alpha(&BLACK));
    let shadow = self.paint_for_shadow(&paint);

    if let Some(image) = &img {
      // remove the positioning from the destination since image_filters.image will return
      // None if the destination left/top is not within the bounds of the original image(!?)
      let mut origin:Point = (dst_rect.left, dst_rect.top).into();
      let resize = Rect::from_size(dst_rect.size());
      let bounds = image.bounds();

      // use an ImageFilter to generate a cropped & scaled version of the original image so
      // we can draw-to-point rather than using draw_image_rect (which would vignette the shadow)
      if let Some(filter) = image_filters::image(image.clone(), Some(src_rect), Some(&resize), paint.filter_quality()){
        if let Some((image, _, dxdy)) = image.new_with_filter(&filter, bounds, bounds){
          let mut canvas = self.surface.as_mut().unwrap().canvas();

          // add the top/left from the original dst_rect back in
          origin.offset(dxdy);

          // draw shadow if applicable
          if let Some(shadow_paint) = shadow{
            canvas.draw_image(&image, origin, Some(&shadow_paint));
          }

          // then draw the actual image
          canvas.draw_image(&image, origin, Some(&paint));
        }
      }
    }
  }

  pub fn blit_image(&mut self, img:&Option<Image>, src_rect:&Rect, dst_rect:&Rect){
    // works just like draw_image but without transforms or shadows
    //
    // BUG: it shouldn't obey they canvas's clipping mask but I haven't figured
    //      out how to cleanly remove then reapply it yet...
    let mut canvas = self.surface.as_mut().unwrap().canvas();
    let mut paint = Paint::default();
    paint.set_style(PaintStyle::Fill);

    if let Some(image) = img{
      canvas.save();
      canvas.reset_matrix();

      canvas.draw_image_rect(&image, Some((src_rect, SrcRectConstraint::Strict)), dst_rect, &paint);
      canvas.restore();
    }
  }

  pub fn choose_font(&mut self, spec: FontSpec){
    // TODO: probably makes sense to share this?
    let mut font_collection = FontCollection::new();
    font_collection.set_default_font_manager(FontMgr::new(), None);

    let faces = font_collection.find_typefaces(&spec.families, spec.style);
    if !faces.is_empty() {
      self.state.font = spec.canonical;
      self.state.char_style.set_font_style(spec.style);
      self.state.char_style.set_font_families(&spec.families);
      self.state.char_style.set_font_size(spec.size);
      self.set_font_variant(&spec.variant, &spec.features);
    }
  }

  pub fn set_font_variant(&mut self, variant:&str, features:&[(String, i32)]){
    self.state.font_variant = variant.to_string();
    self.state.char_style.reset_font_features();
    for (feat, val) in features{
      self.state.char_style.add_font_feature(feat, *val);
    }
  }

  pub fn typeset_text(&mut self, text: &str, paint: Paint) -> Paragraph {
    let mut font_collection = FontCollection::new();
    font_collection.set_default_font_manager(FontMgr::new(), None);

    let mut char_style = self.state.char_style.clone();
    char_style.set_foreground_color(Some(paint));

    let shadow_color = self.color_with_alpha(&self.state.shadow_color);
    let State {shadow_blur, shadow_offset, ..} = self.state;
    let sigma = shadow_blur as f64 / 2.0;
    if shadow_color.a() > 0 && !(shadow_blur == 0.0 && shadow_offset.is_zero()){
      let shadow = TextShadow::new(shadow_color, shadow_offset, sigma);
      char_style.add_shadow(shadow);
    }

    let graf_style = &self.state.graf_style;
    let mut paragraph_builder = ParagraphBuilder::new(&graf_style, font_collection);
    paragraph_builder.push_style(&char_style);
    paragraph_builder.add_text(&text);

    let mut paragraph = paragraph_builder.build();
    paragraph.layout(GALLEY);
    paragraph
  }

  pub fn draw_text(&mut self, text: &str, x: f32, y: f32, paint: Paint){
    let mut paragraph = self.typeset_text(&text, paint);

    let mut point = Point::new(x, y);
    let metrics = self.state.char_style.font_metrics();
    let offset = get_baseline_offset(&metrics, self.state.text_baseline) as f32;
    point.y += offset - paragraph.alphabetic_baseline();
    point.x += GALLEY * get_alignment_factor(&self.state.graf_style);

    let mut canvas = self.surface.as_mut().unwrap().canvas();
    paragraph.paint(canvas, point);
  }

  pub fn measure_text(&mut self, text: &str) -> Vec<f32>{
    let paint = self.paint_for_fill();
    let mut paragraph = self.typeset_text(&text, paint);

    let font_metrics = self.state.char_style.font_metrics();
    let offset = get_baseline_offset(&font_metrics, self.state.text_baseline);
    let hang = offset - get_baseline_offset(&font_metrics, Baseline::Hanging);
    let alph = offset - get_baseline_offset(&font_metrics, Baseline::Alphabetic);
    let ideo = offset - get_baseline_offset(&font_metrics, Baseline::Ideographic);

    let font_ascent = font_metrics.ascent as f64 + offset;
    let font_descent = font_metrics.descent as f64 + offset;
    let em = self.state.char_style.font_size() as f64;

    if let Some(line) = paragraph.get_line_metrics().as_slice().first(){
      vec![
        line.width, line.left, line.width - line.left, line.ascent-offset, line.descent+offset,
        -font_ascent, font_descent, em-font_descent, font_descent,
        hang, alph, ideo
      ].iter().map(|n| *n as f32).collect()
    }else{
      vec![
        0.0, 0.0, 0.0, 0.0, 0.0,
        -font_ascent, font_descent, em-font_descent, font_descent,
        hang, alph, ideo
      ].iter().map(|n| *n as f32).collect()
    }
  }

  pub fn color_with_alpha(&self, src:&Color) -> Color{
    let mut color:Color4f = src.clone().into();
    color.a *= self.state.global_alpha;
    color.to_color()
  }

  pub fn update_image_quality(&mut self){
    self.state.paint.set_filter_quality(match self.state.image_smoothing_enabled{
      true => self.state.image_filter_quality,
      false => FilterQuality::None
    });
  }

  pub fn paint_for_fill(&self) -> Paint{
    let alpha = self.state.global_alpha;
    let dye = &self.state.fill_style;
    let mut paint = self.state.paint.clone();
    paint.set_style(PaintStyle::Fill);
    dye.mix_into(&mut paint, alpha);

    paint
  }

  pub fn paint_for_stroke(&self) -> Paint{
    let alpha = self.state.global_alpha;
    let dye = &self.state.stroke_style;
    let mut paint = self.state.paint.clone();
    paint.set_style(PaintStyle::Stroke);
    dye.mix_into(&mut paint, alpha);

    if !self.state.line_dash_list.is_empty() {
      let dash = dash_path_effect::new(&self.state.line_dash_list, self.state.line_dash_offset);
      paint.set_path_effect(dash);
    }

    paint
  }

  pub fn paint_for_shadow(&self, base_paint:&Paint) -> Option<Paint> {
    let shadow_color = self.color_with_alpha(&self.state.shadow_color);
    let State {shadow_blur, shadow_offset, ..} = self.state;
    let sigma = shadow_blur / 2.0;

    match shadow_color.a() > 0 && !(shadow_blur == 0.0 && shadow_offset.is_zero()){
      true => {
        let mut paint = base_paint.clone();
        if let Some(filter) = image_filters::drop_shadow_only(shadow_offset, (sigma, sigma), shadow_color, None, None){
          paint.set_image_filter(filter);
        }
        Some(paint)
      }
      false => None
    }
  }

  pub fn with_matrix<F>(&mut self, f:F)
    where F:Fn(&mut Matrix) -> &Matrix
  {
    let mut ctm = self.ctm();
    f(&mut ctm);
    let mut canvas = self.surface.as_mut().unwrap().canvas();
    canvas.set_matrix(&ctm);
  }
}


pub fn stash_ref<'a, T: This+Class>(cx: &mut CallContext<'a, T>, queue_name:&str, obj:Handle<'a, JsValue>) -> JsResult<'a, JsUndefined>{
  let this = cx.this().downcast::<JsContext2D>().or_throw(cx)?;
  let sym = symbol(cx, queue_name)?;
  let queue = match this.get(cx, sym)?.downcast::<JsArray>(){
    Ok(array) => array,
    Err(_e) => {
      // create ref queues lazily
      let array = JsArray::new(cx, 0);
      this.set(cx, sym, array)?;
      array
    }
  };

  let depth = cx.borrow(&this, |this| this.state_stack.len() as f64);
  let len = cx.number(depth + 1.0);
  let idx = cx.number(depth);
  let length = cx.string("length");

  queue.set(cx, length, len)?;
  queue.set(cx, idx, obj)?;
  Ok(cx.undefined())
}

pub fn fetch_ref<'a, T: This+Class>(cx: &mut CallContext<'a, T>, queue_name:&str) -> JsResult<'a, JsValue>{
  let this = cx.this().downcast::<JsContext2D>().or_throw(cx)?;
  let sym = symbol(cx, queue_name)?;
  let queue = this.get(cx, sym)?.downcast::<JsArray>().or_throw(cx)?;

  let length = cx.string("length");
  let len = queue.get(cx, length)?.downcast::<JsNumber>().or_throw(cx)?.value() as f64;
  let depth = cx.borrow(&this, |this| this.state_stack.len() as f64);
  let idx = cx.number(depth.min(len - 1.0));

  match queue.get(cx, idx){
    Ok(gradient) => Ok(gradient.upcast()),
    Err(_e) => Ok(cx.undefined().upcast())
  }
}

