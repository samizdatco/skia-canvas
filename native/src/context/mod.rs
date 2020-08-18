#![allow(unused_mut)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]
use std::rc::Rc;
use std::cell::RefCell;
use std::ops::Range;
use neon::prelude::*;
use neon::object::This;
use neon::result::Throw;
use skia_safe::{Canvas as SkCanvas, Surface, Paint, Path, PathOp, Image, ImageInfo, Data,
                Matrix, Rect, Point, IPoint, ISize, Color, Color4f, ColorType,
                PaintStyle, BlendMode, FilterQuality, AlphaType, TileMode, ClipOp,
                image_filters, color_filters, table_color_filter, dash_path_effect};
use skia_safe::textlayout::{Paragraph, ParagraphBuilder, ParagraphStyle, TextStyle, TextShadow};
use skia_safe::canvas::SrcRectConstraint;
use skia_safe::path::FillType;

use crate::utils::*;
use crate::typography::*;
use crate::gradient::{CanvasGradient, JsCanvasGradient};
use crate::pattern::{CanvasPattern, JsCanvasPattern};

const BLACK:Color = Color::BLACK;
const TRANSPARENT:Color = Color::TRANSPARENT;
const GALLEY:f32 = 100_000.0;

pub mod class;
pub use class::JsContext2D;

pub struct Context2D{
  surface: Rc<RefCell<Surface>>,
  library: Rc<RefCell<FontLibrary>>,
  path: Path,
  state_stack: Vec<State>,
  state: State,
}

#[derive(Clone)]
pub struct State{
  clip: Path,
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

  global_alpha: f32,
  global_composite_operation: BlendMode,
  image_filter_quality: FilterQuality,
  image_smoothing_enabled: bool,
  filter:String,

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
    paint.set_stroke_miter(10.0);
    paint.set_color(BLACK);
    paint.set_anti_alias(true);
    paint.set_stroke_width(1.0);
    paint.set_filter_quality(FilterQuality::Low);

    let graf_style = ParagraphStyle::new();
    let mut char_style = TextStyle::new();
    char_style.set_font_size(10.0);

    State {
      paint,
      clip: Path::new(),
      matrix: Matrix::new_identity(),
      stroke_style: Dye::Color(BLACK),
      fill_style: Dye::Color(BLACK),

      stroke_width: 1.0,
      line_dash_offset: 0.0,
      line_dash_list: vec![],

      global_alpha: 1.0,
      global_composite_operation: BlendMode::SrcOver,
      image_filter_quality: FilterQuality::Low,
      image_smoothing_enabled: true,
      filter: "none".to_string(),

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
      text_wrap: false
    }
  }
}

impl Context2D{
  pub fn new(surface: &Rc<RefCell<Surface>>, library: &Rc<RefCell<FontLibrary>>) -> Self {
    Context2D{
      surface: Rc::clone(&surface),
      library: Rc::clone(&library),
      path: Path::new(),
      state_stack: vec![],
      state: State::default(),
    }
  }

  pub fn ctm(&mut self) -> Matrix {
    let mut surface = self.surface.borrow_mut();
    let canvas = surface.canvas();
    canvas.total_matrix()
  }

  pub fn in_local_coordinates(&mut self, x: f32, y: f32) -> Point{
    match self.state.matrix.invert(){
      Some(inverse) => inverse.map_point((x, y)),
      None => (x, y).into()
    }
  }

  pub fn with_canvas<F>(&self, f:F)
    where F:FnOnce(&mut SkCanvas)
  {
    let mut surface = self.surface.borrow_mut();
    f(surface.canvas());
  }

  pub fn with_matrix<F>(&mut self, f:F)
    where F:FnOnce(&mut Matrix) -> &Matrix
  {
    f(&mut self.state.matrix);
    self.with_canvas(|canvas| {
      canvas.set_matrix(&self.state.matrix);
    });
  }

  pub fn reset_canvas(&mut self){
    // clears the active clip and transform from the canvas (but not from the state struct)
    self.with_canvas(|canvas|{
      canvas.restore_to_count(1);
      canvas.save();
    });
  }

  pub fn reset_state(&mut self) {
    // called when the canvas gets resized
    self.path = Path::new();
    self.state_stack = vec![];
    self.state = State::default();
    self.reset_canvas();
  }

  pub fn push(&mut self){
    let new_state = self.state.clone();
    self.state_stack.push(new_state);
  }

  pub fn pop(&mut self){
    // don't do anything if we're already back at the initial stack frame
    if let Some(old_state) = self.state_stack.pop(){
      self.state = old_state;

      self.reset_canvas();
      self.with_canvas(|canvas|{
        canvas.set_matrix(&self.state.matrix);
        if !self.state.clip.is_empty(){
          canvas.clip_path(&self.state.clip, ClipOp::Intersect, true /* antialias */);
        }
      });
    }

  }

  pub fn draw_path(&mut self, paint: &Paint){
    self.with_canvas(|canvas|{
      // draw shadow if applicable
      let shadow = self.paint_for_shadow(&paint);
      if let Some(shadow_paint) = shadow{
        canvas.draw_path(&self.path, &shadow_paint);
      }

      // then draw the actual path
      canvas.draw_path(&self.path, &paint);
    });
  }

  pub fn clip_path(&mut self, path: Option<Path>, rule:FillType){
    let mut clip = match path{
      Some(path) => path,
      None => self.path.clone()
    };

    clip.set_fill_type(rule);
    if self.state.clip.is_empty(){
      self.state.clip = clip.clone();
    }else if let Some(new_clip) = self.state.clip.op(&clip, PathOp::Intersect){
      self.state.clip = new_clip;
    }

    let do_aa = true;
    self.with_canvas(|canvas| {
      canvas.clip_path(&clip, ClipOp::Intersect, do_aa);
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
    self.with_canvas(|canvas| {
      let shadow = self.paint_for_shadow(&paint);

      // draw shadow if applicable
      if let Some(shadow_paint) = shadow{
        canvas.draw_rect(&rect, &shadow_paint);
      }

      // then draw the actual rect
      canvas.draw_rect(&rect, &paint);
    });
  }

  pub fn clear_rect(&mut self, rect:&Rect){
    self.with_canvas(|canvas| {
      let mut paint = Paint::default();
      paint.set_style(PaintStyle::Fill);
      paint.set_blend_mode(BlendMode::Clear);
      canvas.draw_rect(&rect, &paint);
    });
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
          self.with_canvas(|canvas| {
            // add the top/left from the original dst_rect back in
            origin.offset(dxdy);

            // draw shadow if applicable
            if let Some(shadow_paint) = shadow{
              canvas.draw_image(&image, origin, Some(&shadow_paint));
            }

            // then draw the actual image
            canvas.draw_image(&image, origin, Some(&paint));
          });
        }
      }
    }
  }

  pub fn get_pixels(&mut self, buffer: &mut [u8], origin: impl Into<IPoint>, size: impl Into<ISize>){
    let info = ImageInfo::new(size, ColorType::RGBA8888, AlphaType::Unpremul, None);
    let mut surface = self.surface.borrow_mut();
    surface.read_pixels(&info, buffer, info.min_row_bytes(), origin);
  }

  pub fn blit_pixels(&mut self, buffer: &[u8], info: &ImageInfo, src_rect:&Rect, dst_rect:&Rect){
    // works just like draw_image in terms of src/dst rects, but without the clips, transforms, or shadows
    let data = unsafe{ Data::new_bytes(buffer) };

    if let Some(bitmap) = Image::from_raster_data(&info, data, info.min_row_bytes()) {
      let mut paint = Paint::default();
      paint.set_style(PaintStyle::Fill);

      self.push();
      self.reset_canvas();
      self.with_canvas(|canvas| {
        canvas.draw_image_rect(&bitmap, Some((src_rect, SrcRectConstraint::Strict)), dst_rect, &paint);
      });
      self.pop();
    }
  }

  pub fn set_font(&mut self, spec: FontSpec){
    let mut library = self.library.borrow_mut();
    if let Some(new_style) = library.update_style(&self.state.char_style, &spec){
      self.state.font = spec.canonical;
      self.state.font_variant = spec.variant.to_string();
      self.state.char_style = new_style;
    }
  }

  pub fn set_font_variant(&mut self, variant:&str, features:&[(String, i32)]){
    let mut library = self.library.borrow_mut();
    let new_style = library.update_features(&self.state.char_style, features);
    self.state.font_variant = variant.to_string();
    self.state.char_style = new_style;
  }

  pub fn typeset(&mut self, text: &str, width:f32, paint: Paint) -> Paragraph {
    let mut char_style = self.state.char_style.clone();
    char_style.set_foreground_color(Some(paint));

    let shadow_color = self.color_with_alpha(&self.state.shadow_color);
    let State {shadow_blur, shadow_offset, ..} = self.state;
    let sigma = shadow_blur as f64 / 2.0;
    if shadow_color.a() > 0 && !(shadow_blur == 0.0 && shadow_offset.is_zero()){
      let shadow = TextShadow::new(shadow_color, shadow_offset, sigma);
      char_style.add_shadow(shadow);
    }

    let mut graf_style = self.state.graf_style.clone();
    let text = match self.state.text_wrap{
      true => text.to_string(),
      false => {
        graf_style.set_max_lines(1);
        text.replace("\n", " ")
      }
    };

    let mut library = self.library.borrow_mut();
    let collection = library.collect_fonts(&char_style);
    let mut paragraph_builder = ParagraphBuilder::new(&graf_style, collection);
    paragraph_builder.push_style(&char_style);
    paragraph_builder.add_text(&text);

    let mut paragraph = paragraph_builder.build();
    paragraph.layout(width);
    paragraph
  }

  pub fn draw_text(&mut self, text: &str, x: f32, y: f32, width: Option<f32>, paint: Paint){
    let width = width.unwrap_or(GALLEY);
    let mut paragraph = self.typeset(&text, width, paint);
    let mut point = Point::new(x, y);
    let metrics = self.state.char_style.font_metrics();
    let offset = get_baseline_offset(&metrics, self.state.text_baseline);
    point.y += offset - paragraph.alphabetic_baseline();
    point.x += width * get_alignment_factor(&self.state.graf_style);

    self.with_canvas(|canvas| {
      paragraph.paint(canvas, point);
    });
  }

  pub fn measure_text(&mut self, text: &str, width:Option<f32>) -> Vec<Vec<f32>>{
    let paint = self.paint_for_fill();
    let mut paragraph = self.typeset(&text, width.unwrap_or(GALLEY), paint);

    let font_metrics = self.state.char_style.font_metrics();
    let offset = get_baseline_offset(&font_metrics, self.state.text_baseline);
    let hang = get_baseline_offset(&font_metrics, Baseline::Hanging) - offset;
    let norm = get_baseline_offset(&font_metrics, Baseline::Alphabetic) - offset;
    let ideo = get_baseline_offset(&font_metrics, Baseline::Ideographic) - offset;
    let ascent = norm - font_metrics.ascent;
    let descent = font_metrics.descent - norm;
    let alignment = get_alignment_factor(&self.state.graf_style);

    if paragraph.line_number() == 0 {
      return vec![vec![0.0, 0.0, 0.0, 0.0, 0.0, ascent, descent, ascent, descent, hang, norm, ideo]]
    }

    // find the bounds and text-range for each individual line
    let origin = paragraph.get_line_metrics()[0].baseline;
    let line_rects:Vec<(Rect, Range<usize>, f32)> = paragraph.get_line_metrics().iter().map(|line|{
      let baseline = line.baseline - origin;
      let rect = Rect::new(line.left as f32, (baseline - line.ascent) as f32,
                          (line.width - line.left) as f32, (baseline + line.descent) as f32);
      let range = string_idx_range(text, line.start_index, line.end_excluding_whitespaces);
      (rect.with_offset((alignment*rect.width(), offset)), range, baseline as f32)
    }).collect();

    // take their union to find the bounds for the whole text run
    let (bounds, chars) = line_rects.iter().fold((Rect::new_empty(), 0), |(union, indices), (rect, range, _)|
      (Rect::join2(union, rect), range.end)
    );

    // return a list-of-lists whose first entry is the whole-run font metrics and subsequent entries are
    // line-rect/range values (with the js side responsible for restructuring the whole bundle)
    let mut results = vec![vec![
      bounds.width(), bounds.left, bounds.right, -bounds.top, bounds.bottom,
      ascent, descent, ascent, descent, hang, norm, ideo
    ]];
    line_rects.iter().for_each(|(rect, range, baseline)|{
      results.push(vec![rect.left, rect.top, rect.width(), rect.height(),
                        *baseline, range.start as f32, range.end as f32])
    });
    results
  }

  pub fn set_filter(&mut self, filter_text:&str, specs:&[FilterSpec]){
    // matrices and formulæ taken from: https://www.w3.org/TR/filter-effects-1/
    let filter = specs.iter().fold(None, |chain, next_filter|
      match next_filter {
        FilterSpec::Shadow{ offset, blur, color } => {
          let sigma = *blur / 2.0;
          image_filters::drop_shadow(*offset, (sigma, sigma), *color, chain, None)
        },
        FilterSpec::Plain{ name, value } => match name.as_ref() {
          "blur" => {
            image_filters::blur((*value, *value), TileMode::Clamp, chain, None)
          },
          "brightness" => {
            let amt = value.max(0.0);
            let color_matrix = color_filters::matrix_row_major(&[
              amt,  0.0,  0.0,  0.0, 0.0,
              0.0,  amt,  0.0,  0.0, 0.0,
              0.0,  0.0,  amt,  0.0, 0.0,
              0.0,  0.0,  0.0,  1.0, 0.0
            ]);
            image_filters::color_filter(color_matrix, chain, None)
          },
          "contrast" => {
            let amt = value.max(0.0);
            let mut ramp = [0u8; 256];
            for (i, val) in ramp.iter_mut().take(256).enumerate() {
              let orig = i as f32;
              *val = (127.0 + amt * orig - (127.0 * amt )) as u8;
            }
            let table = Some(&ramp);
            let color_table = table_color_filter::from_argb(None, table, table, table);
            image_filters::color_filter(color_table, chain, None)
          },
          "grayscale" => {
            let amt = 1.0 - value.max(0.0).min(1.0);
            let color_matrix = color_filters::matrix_row_major(&[
              (0.2126 + 0.7874 * amt), (0.7152 - 0.7152  * amt), (0.0722 - 0.0722 * amt), 0.0, 0.0,
              (0.2126 - 0.2126 * amt), (0.7152 + 0.2848  * amt), (0.0722 - 0.0722 * amt), 0.0, 0.0,
              (0.2126 - 0.2126 * amt), (0.7152 - 0.7152  * amt), (0.0722 + 0.9278 * amt), 0.0, 0.0,
               0.0,                     0.0,                      0.0,                    1.0, 0.0
            ]);
            image_filters::color_filter(color_matrix, chain, None)
          },
          "invert" => {
            let amt = value.max(0.0).min(1.0);
            let mut ramp = [0u8; 256];
            for (i, val) in ramp.iter_mut().take(256).enumerate().map(|(i,v)| (i as f32, v)) {
              let (orig, inv) = (i, 255.0-i);
              *val = (orig * (1.0 - amt) + inv * amt) as u8;
            }
            let table = Some(&ramp);
            let color_table = table_color_filter::from_argb(None, table, table, table);
            image_filters::color_filter(color_table, chain, None)
          },
          "opacity" => {
            let amt = value.max(0.0).min(1.0);
            let color_matrix = color_filters::matrix_row_major(&[
              1.0,  0.0,  0.0,  0.0,  0.0,
              0.0,  1.0,  0.0,  0.0,  0.0,
              0.0,  0.0,  1.0,  0.0,  0.0,
              0.0,  0.0,  0.0,  amt,  0.0
            ]);
            image_filters::color_filter(color_matrix, chain, None)
          },
          "saturate" => {
            let amt = value.max(0.0);
            let color_matrix = color_filters::matrix_row_major(&[
              (0.2126 + 0.7874 * amt), (0.7152 - 0.7152 * amt), (0.0722 - 0.0722 * amt), 0.0, 0.0,
              (0.2126 - 0.2126 * amt), (0.7152 + 0.2848 * amt), (0.0722 - 0.0722 * amt), 0.0, 0.0,
              (0.2126 - 0.2126 * amt), (0.7152 - 0.7152 * amt), (0.0722 + 0.9278 * amt), 0.0, 0.0,
               0.0,                     0.0,                     0.0,                    1.0, 0.0
            ]);
            image_filters::color_filter(color_matrix, chain, None)
          },
          "sepia" => {
            let amt = 1.0 - value.max(0.0).min(1.0);
            let color_matrix = color_filters::matrix_row_major(&[
              (0.393 + 0.607 * amt), (0.769 - 0.769 * amt), (0.189 - 0.189 * amt), 0.0, 0.0,
              (0.349 - 0.349 * amt), (0.686 + 0.314 * amt), (0.168 - 0.168 * amt), 0.0, 0.0,
              (0.272 - 0.272 * amt), (0.534 - 0.534 * amt), (0.131 + 0.869 * amt), 0.0, 0.0,
               0.0,                   0.0,                   0.0,                  1.0, 0.0
            ]);
            image_filters::color_filter(color_matrix, chain, None)
          },
          "hue-rotate" => {
            let cos = to_radians(*value).cos();
            let sin = to_radians(*value).sin();
            let color_matrix = color_filters::matrix_row_major(&[
              (0.213 + cos*0.787 - sin*0.213), (0.715 - cos*0.715 - sin*0.715), (0.072 - cos*0.072 + sin*0.928), 0.0, 0.0,
              (0.213 - cos*0.213 + sin*0.143), (0.715 + cos*0.285 + sin*0.140), (0.072 - cos*0.072 - sin*0.283), 0.0, 0.0,
              (0.213 - cos*0.213 - sin*0.787), (0.715 - cos*0.715 + sin*0.715), (0.072 + cos*0.928 + sin*0.072), 0.0, 0.0,
               0.0,                             0.0,                             0.0,                            1.0, 0.0
            ]);
            image_filters::color_filter(color_matrix, chain, None)
          },
          _ => chain
        }
      }
    );

    self.state.paint.set_image_filter(filter);
    self.state.filter = filter_text.to_string();
  }

  pub fn update_image_quality(&mut self){
    self.state.paint.set_filter_quality(match self.state.image_smoothing_enabled{
      true => self.state.image_filter_quality,
      false => FilterQuality::None
    });
  }

  pub fn color_with_alpha(&self, src:&Color) -> Color{
    let mut color:Color4f = src.clone().into();
    color.a *= self.state.global_alpha;
    color.to_color()
  }

  pub fn paint_for_fill(&self) -> Paint{
    let mut paint = self.state.paint.clone();
    paint.set_style(PaintStyle::Fill);

    let dye = &self.state.fill_style;
    let alpha = self.state.global_alpha;
    dye.mix_into(&mut paint, alpha);

    paint
  }

  pub fn paint_for_stroke(&self) -> Paint{
    let mut paint = self.state.paint.clone();
    paint.set_style(PaintStyle::Stroke);

    let dye = &self.state.stroke_style;
    let alpha = self.state.global_alpha;
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
          paint.set_image_filter(filter); // this also knocks out any ctx.filter settings as a side-effect
        }
        Some(paint)
      }
      false => None
    }
  }

}

//
// Dye abstraction for Color / CanvasGradient / CanvasPattern
//

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

// -- persistent references to js gradient/pattern objects ------------------------------

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

