#![allow(unused_mut)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]
use std::f32::consts::PI;
use neon::prelude::*;
use neon::object::This;
use skia_safe::{Surface, Canvas, Path, Matrix, Paint, Rect, Point, Color, Color4f, ImageInfo,
                PaintStyle, BlendMode, FilterQuality, MaskFilter, BlurStyle, PathDirection,
                Data, Image, EncodedImageFormat, Font, dash_path_effect, ClipOp, image_filters,
                utils::text_utils::Align, path::{AddPathMode, FillType}, canvas::SrcRectConstraint,
                ColorType, AlphaType};

use crate::utils::*;
use crate::path2d::{Path2D, JsPath2D};
use crate::image::{JsImage, JsImageData};
use crate::gradient::{CanvasGradient, JsCanvasGradient};
use crate::pattern::{CanvasPattern, JsCanvasPattern};

const BLACK:Color = Color::BLACK;
const TRANSPARENT:Color = Color::TRANSPARENT;

pub struct Context2D{
  surface: Option<Surface>,
  path: Path,
  font: Option<Font>, // for now
  state_stack: Vec<State>,
  state: State,
}

#[derive(Clone)]
pub struct State{
  paint: Paint,

  fill_style: Dye,
  stroke_style: Dye,
  shadow_blur: f32,
  shadow_color: Color,
  shadow_offset: Point,

  global_alpha: f32,
  stroke_width: f32,
  line_dash_offset: f32,
  line_dash_list: Vec<f32>,

  global_composite_operation: BlendMode,
  image_filter_quality: FilterQuality,
  image_smoothing_enabled: bool,

  font_string: String,
  text_ltr: bool,
  text_align: Align,
  text_baseline: Baseline,
}

#[derive(Clone)]
pub enum Dye{
  Color(Color),
  Gradient(CanvasGradient),
  Pattern(CanvasPattern)
}

impl Context2D{
  pub fn ctm(&mut self) -> Matrix {
    match self.surface.as_mut() {
      Some(surface) => surface.canvas().total_matrix(),
      None => Matrix::new_identity()
    }
  }

  pub fn in_local_coordinates(&mut self, x: f32, y: f32) -> Point{
    match self.ctm().invert(){
      Some(inverse) => inverse.map_point((x, y)),
      None => (x, y).into()
    }
  }

  pub fn push(&mut self){
    let new_state = self.state.clone();
    self.state_stack.push(new_state);
    if let Some(surface) = self.surface.as_mut(){
      surface.canvas().save();
    }
  }

  pub fn pop(&mut self){
    if let Some(old_state) = self.state_stack.pop(){
      self.state = old_state;
    }
    if let Some(surface) = self.surface.as_mut(){
      surface.canvas().restore();
    }
  }

  pub fn draw_path(&mut self, paint: &Paint){
    let shadow = self.paint_for_shadow(&paint);

    if let Some(surface) = &mut self.surface{
      // draw shadow if applicable
      if let Some(shadow_paint) = shadow{
        surface.canvas().draw_path(&self.path, &shadow_paint);
      }

      // then draw the actual path
      surface.canvas().draw_path(&self.path, &paint);
    }
  }

  pub fn clip_path(&mut self, path: Option<Path>, rule:FillType){
    let do_aa = true;
    let mut clip = match path{
      Some(path) => path,
      None => self.path.clone()
    };

    clip.set_fill_type(rule);
    if let Some(surface) = &mut self.surface{
      let canvas = surface.canvas();
      canvas.clip_path(&clip, ClipOp::Intersect, do_aa);
    }
  }

  pub fn draw_rect(&mut self, rect:&Rect, paint: &Paint){
    let shadow = self.paint_for_shadow(&paint);

    if let Some(surface) = &mut self.surface{
      // draw shadow if applicable
      if let Some(shadow_paint) = shadow{
        surface.canvas().draw_rect(&rect, &shadow_paint);
      }

      // then draw the actual rect
      surface.canvas().draw_rect(&rect, &paint);
    }
  }

  pub fn clear_rect(&mut self, rect:&Rect){
    let mut paint = Paint::default();
    paint.set_style(PaintStyle::Fill);
    paint.set_blend_mode(BlendMode::Clear);

    if let Some(surface) = &mut self.surface{
      surface.canvas().draw_rect(&rect, &paint);
    }
  }

  pub fn draw_image(&mut self, img:&Option<Image>, src_rect:&Rect, dst_rect:&Rect){
    let mut paint = self.state.paint.clone();
    paint.set_style(PaintStyle::Fill);
    paint.set_color(self.color_with_alpha(&BLACK));

    // use an ImageFilter to generate a cropped & scaled version of the
    // original image then draw-to-point rather than using draw_image_rect
    if let Some(image) = &img{
      let shadow = self.paint_for_shadow(&paint);
      let bounds = image.bounds();
      if let Some(filter) = image_filters::image(image.clone(), Some(src_rect), Some(dst_rect), paint.filter_quality()){
        if let Some((image, bounds, origin)) = image.new_with_filter(&filter, bounds, bounds){
          if let Some(surface) = &mut self.surface{
            // draw shadow if applicable
            if let Some(shadow_paint) = shadow{
              surface.canvas().draw_image(&image, origin, Some(&shadow_paint));
            }
            // then draw the actual image
            surface.canvas().draw_image(&image, origin, Some(&paint));
          }
        }
      }
    }
  }

  pub fn blit_image(&mut self, img:&Option<Image>, src_rect:&Rect, dst_rect:&Rect){
    // works just like draw_image but without transforms or shadows
    //
    // BUG: it shouldn't obey they canvas's clipping mask but I haven't figured
    //      out how to cleanly remove then reapply it yet...
    let mut paint = Paint::default();
    paint.set_style(PaintStyle::Fill);

    if let Some(image) = img{
      if let Some(surface) = &mut self.surface{
        let canvas = surface.canvas();
        canvas.save();
        canvas.reset_matrix();

        canvas.draw_image_rect(&image, Some((src_rect, SrcRectConstraint::Strict)), dst_rect, &paint);
        canvas.restore();
      }
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
    let mut paint = self.state.paint.clone();
    paint.set_style(PaintStyle::Fill);

    match &self.state.fill_style{
      Dye::Color(color) => { paint.set_color(self.color_with_alpha(&color)); },
      Dye::Gradient(gradient) => {paint.set_shader(gradient.shader());},
      Dye::Pattern(pattern) => {paint.set_shader(pattern.shader());}
    }

    paint
  }

  pub fn paint_for_stroke(&self) -> Paint{
    let mut paint = self.state.paint.clone();
    paint.set_style(PaintStyle::Stroke);

    match &self.state.stroke_style{
      Dye::Color(color) => { paint.set_color(self.color_with_alpha(&color)); },
      Dye::Gradient(gradient) => {paint.set_shader(gradient.shader());},
      Dye::Pattern(pattern) => {paint.set_shader(pattern.shader());}
    }

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
    if let Some(surface) = &mut self.surface{
      surface.canvas().set_matrix(&ctm);
    }
  }

}

pub fn stash_ref<'a, T: This+Class>(cx: &mut CallContext<'a, T>, queue_name:&str, obj:Handle<'a, JsValue>) -> JsResult<'a, JsUndefined>{
  let mut this = cx.this().downcast::<JsContext2D>().or_throw(cx)?;
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
  let mut this = cx.this().downcast::<JsContext2D>().or_throw(cx)?;
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

declare_types! {
  pub class JsContext2D for Context2D {
    init(_) {
      let mut paint = Paint::default();
      paint.set_stroke_miter(10.0);
      paint.set_color(BLACK);
      paint.set_anti_alias(true);
      paint.set_stroke_width(1.0);
      paint.set_filter_quality(FilterQuality::Low);

      Ok( Context2D{
        surface: None,
        path: Path::new(),
        font: None,
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

          font_string: "10px monospace".to_string(),
          text_ltr: true,
          text_align: Align::Left,
          text_baseline: Baseline::Alphabetic,
        },
      })
    }

    constructor(mut cx){
      let mut this = cx.this();
      let width = float_arg(&mut cx, 0, "width")?;
      let height = float_arg(&mut cx, 1, "height")?;
      if width > 0.0 && height > 0.0 {
        cx.borrow_mut(&mut this, |mut this| {
          this.surface = Some(Surface::new_raster_n32_premul((width as i32, height as i32)).expect("no surface!"));
        });
      }else{
        return cx.throw_error("width and height must be greater than zero")
      }

      Ok(None)
    }

    /* ---------------------------------------------------------------------- *
     |                                METHODS                                 |
     * ---------------------------------------------------------------------- */

    //
    // State
    //

    method save(mut cx){
      let mut this = cx.this();
      let depth = cx.borrow_mut(&mut this, |mut this| this.push() );
      Ok(cx.undefined().upcast())
    }

    method restore(mut cx){
      let mut this = cx.this();
      let depth = cx.borrow_mut(&mut this, |mut this| this.pop() );
      Ok(cx.undefined().upcast())
    }

    method clip(mut cx){
      let mut this = cx.this();

      let mut shift = 0;
      let clip = path2d_arg_opt(&mut cx, 0);
      if clip.is_some() { shift += 1; }

      let rule = fill_rule_arg_or(&mut cx, shift, "nonzero")?;
      cx.borrow_mut(&mut this, |mut this| { this.clip_path(clip, rule); });
      Ok(cx.undefined().upcast())
    }

    method getLineDash(mut cx){
      let mut this = cx.this();
      let dashes = cx.borrow(&this, |this| this.state.line_dash_list.clone());
      let array = JsArray::new(&mut cx, dashes.len() as u32);
      for (i, interval) in dashes.iter().enumerate() {
        let num = cx.number(*interval);
        array.set(&mut cx, i as u32, num)?;
      }
      Ok(array.upcast())
    }

    method setLineDash(mut cx){
      let mut this = cx.this();
      if !cx.argument::<JsValue>(0)?.is_a::<JsArray>(){
        return cx.throw_type_error("Value is not a sequence")
      } else {
        let list = cx.argument::<JsArray>(0)?.to_vec(&mut cx)?;
        let intervals = floats_in(&list);
        let success = cx.borrow_mut(&mut this, |mut this| {
          this.state.line_dash_list = intervals
        });
      }
      Ok(cx.undefined().upcast())
    }

    //
    // Matrix
    //
    // Implemented in js:
    // - getTransform
    // - setTransform

    method transform(mut cx){
      let mut this = cx.this();
      let matrix = matrix_args(&mut cx, 0..6)?;
      cx.borrow_mut(&mut this, |mut this| {
        this.with_matrix(|ctm| ctm.pre_concat(&matrix) );
      });
      Ok(cx.undefined().upcast())
    }

    method translate(mut cx){
      let mut this = cx.this();
      let dx = float_arg(&mut cx, 0, "deltaX")?;
      let dy = float_arg(&mut cx, 0, "deltaY")?;
      cx.borrow_mut(&mut this, |mut this| {
        this.with_matrix(|ctm| ctm.pre_translate((dx, dy)) );
      });
      Ok(cx.undefined().upcast())
    }

    method scale(mut cx){
      let mut this = cx.this();
      let x_scale = float_arg(&mut cx, 0, "xScale")?;
      let y_scale = float_arg(&mut cx, 0, "yScale")?;
      cx.borrow_mut(&mut this, |mut this| {
        this.with_matrix(|ctm| ctm.pre_scale((x_scale, y_scale), None) );
      });
      Ok(cx.undefined().upcast())
    }

    method rotate(mut cx){
      let mut this = cx.this();
      let radians = float_arg(&mut cx, 0, "angle")?;
      let degrees = radians / PI * 180.0;
      cx.borrow_mut(&mut this, |mut this| {
        this.with_matrix(|ctm| ctm.pre_rotate(degrees, None) );
      });
      Ok(cx.undefined().upcast())
    }

    method resetTransform(mut cx){
      let mut this = cx.this();
      cx.borrow_mut(&mut this, |mut this|
        this.with_matrix(|ctm| ctm.reset() )
      );
      Ok(cx.undefined().upcast())
    }

    //
    // Paths
    //

    method beginPath(mut cx){
      let mut this = cx.this();
      cx.borrow_mut(&mut this, |mut this| {
        this.path = Path::new();
      });
      Ok(cx.undefined().upcast())
    }

    method rect(mut cx){
      let mut this = cx.this();
      let nums = float_args(&mut cx, 0..4)?;
      if let [x, y, w, h] = nums.as_slice(){
        let rect = Rect::from_xywh(*x, *y, *w, *h);
        cx.borrow_mut(&mut this, |mut this| {
          this.path.add_rect(rect, Some((PathDirection::CW, 0)));
        });
      }

      Ok(cx.undefined().upcast())
    }

    method arc(mut cx){
      let mut this = cx.this();
      let nums = float_args(&mut cx, 0..5)?;
      let ccw = bool_arg_or(&mut cx, 5, false);

      if let [x, y, radius, start_angle, end_angle] = nums.as_slice(){
        cx.borrow_mut(&mut this, |mut this| {
          let mut arc = Path2D::new();
          arc.add_ellipse((*x, *y), (*radius, *radius), 0.0, *start_angle, *end_angle, ccw);
          this.path.add_path(&arc.path, (0,0), AddPathMode::Append);
        });
      }
      Ok(cx.undefined().upcast())
    }

    method ellipse(mut cx){
      let mut this = cx.this();
      let nums = float_args(&mut cx, 0..7)?;
      let ccw = bool_arg(&mut cx, 7, "isCCW")?;

      if let [x, y, x_radius, y_radius, rotation, start_angle, end_angle] = nums.as_slice(){
        if *x_radius < 0.0 || *y_radius < 0.0 {
          return cx.throw_error("radii cannot be negative")
        }
        cx.borrow_mut(&mut this, |mut this| {
          let mut arc = Path2D::new();
          arc.add_ellipse((*x, *y), (*x_radius, *y_radius), *rotation, *start_angle, *end_angle, ccw);
          this.path.add_path(&arc.path, (0,0), AddPathMode::Append);
        });
      }

      Ok(cx.undefined().upcast())
    }


    method moveTo(mut cx){
      let mut this = cx.this();
      let x = float_arg(&mut cx, 0, "x")?;
      let y = float_arg(&mut cx, 1, "y")?;
      cx.borrow_mut(&mut this, |mut this| {
        this.path.move_to((x, y));
      });
      Ok(cx.undefined().upcast())
    }

    method lineTo(mut cx){
      let mut this = cx.this();
      let x = float_arg(&mut cx, 0, "x")?;
      let y = float_arg(&mut cx, 1, "y")?;
      cx.borrow_mut(&mut this, |mut this| {
        if this.path.is_empty(){ this.path.move_to((x, y)); }
        this.path.line_to((x, y));
      });
      Ok(cx.undefined().upcast())
    }

    method arcTo(mut cx){
      let mut this = cx.this();
      let coords = float_args(&mut cx, 0..4)?;
      let radius = float_arg(&mut cx, 4, "radius")?;

      if let [x1, y1, x2, y2] = coords.as_slice(){
        cx.borrow_mut(&mut this, |mut this| {
          if this.path.is_empty(){ this.path.move_to((*x1, *y1)); }
          this.path.arc_to_tangent((*x1, *y1), (*x2, *y2), radius);
        });
      }
      Ok(cx.undefined().upcast())
    }

    method bezierCurveTo(mut cx){
      let mut this = cx.this();
      let nums = float_args(&mut cx, 0..6)?;
      if let [cp1x, cp1y, cp2x, cp2y, x, y] = nums.as_slice(){
        cx.borrow_mut(&mut this, |mut this| {
          if this.path.is_empty(){ this.path.move_to((*cp1x, *cp1y)); }
          this.path.cubic_to((*cp1x, *cp1y), (*cp2x, *cp2y), (*x, *y));
        });
      }
      Ok(cx.undefined().upcast())
    }

    method quadraticCurveTo(mut cx){
      let mut this = cx.this();
      let nums = float_args(&mut cx, 0..4)?;
      if let [cpx, cpy, x, y] = nums.as_slice(){
        cx.borrow_mut(&mut this, |mut this| {
          if this.path.is_empty(){ this.path.move_to((*cpx, *cpy)); }
          this.path.quad_to((*cpx, *cpy), (*x, *y));
        });
      }
      Ok(cx.undefined().upcast())
    }

    method closePath(mut cx){
      let mut this = cx.this();
      cx.borrow_mut(&mut this, |mut this| {
        this.path.close();
      });
      Ok(cx.undefined().upcast())
    }

    method isPointInPath(mut cx){
      let mut this = cx.this();
      let (mut container, shift) = match cx.argument::<JsValue>(0)?.is_a::<JsPath2D>(){
        true => (cx.argument(0)?, 1),
        false => (this, 0)
      };
      let x = float_arg(&mut cx, shift, "x")?;
      let y = float_arg(&mut cx, shift+1, "y")?;
      let rule = fill_rule_arg_or(&mut cx, shift+2, "nonzero")?;

      let point = cx.borrow_mut(&mut this, |mut this| this.in_local_coordinates(x, y) );
      let contained = cx.borrow_mut(&mut container, |mut obj| {
        let prev_rule = obj.path.fill_type();
        obj.path.set_fill_type(rule);
        let is_in = obj.path.contains(point);
        obj.path.set_fill_type(prev_rule);
        is_in
      });
      Ok(cx.boolean(contained).upcast())
    }

    method isPointInStroke(mut cx){
      let mut this = cx.this();
      let (mut container, shift) = match cx.argument::<JsValue>(0)?.is_a::<JsPath2D>(){
        true => (cx.argument(0)?, 1),
        false => (this, 0)
      };
      let x = float_arg(&mut cx, shift, "x")?;
      let y = float_arg(&mut cx, shift+1, "y")?;
      let point = cx.borrow_mut(&mut this, |mut this| this.in_local_coordinates(x, y) );

      let paint = cx.borrow(&this, |this| this.state.paint.clone() );
      let precision = 0.3; // this is what Chrome uses to compute this
      let contained = match cx.borrow(&container, |obj| paint.get_fill_path(&obj.path, None, Some(precision)) ){
        Some(mut outline) => {
          outline.set_fill_type(FillType::Winding);
          outline.contains(point)
        }
        None => false
      };

      Ok(cx.boolean(contained).upcast())
    }

    //
    // Drawing
    //
    method fill(mut cx){
      let mut this = cx.this();

      let mut shift = 0;
      if let Some(path) = path2d_arg_opt(&mut cx, 0){
        cx.borrow_mut(&mut this, |mut this| { this.path = path });
        shift += 1;
      }
      let rule = fill_rule_arg_or(&mut cx, shift, "nonzero")?;

      cx.borrow_mut(&mut this, |mut this| {
        let paint = this.paint_for_fill();
        this.path.set_fill_type(rule);
        this.draw_path(&paint);
      });

      Ok(cx.undefined().upcast())
    }

    method stroke(mut cx){
      let mut this = cx.this();
      if let Some(path) = path2d_arg_opt(&mut cx, 0){
        cx.borrow_mut(&mut this, |mut this| { this.path = path });
      }

      cx.borrow_mut(&mut this, |mut this| {
        let paint = this.paint_for_stroke();
        this.draw_path(&paint);
      });

      Ok(cx.undefined().upcast())
    }

    method fillRect(mut cx){
      let mut this = cx.this();
      let nums = float_args(&mut cx, 0..4)?;
      if let [x, y, w, h] = nums.as_slice() {
        let rect = Rect::from_xywh(*x, *y, *w, *h);
        cx.borrow_mut(&mut this, |mut this| {
          let paint =  this.paint_for_fill();
          this.draw_rect(&rect, &paint);

        })
      }
      Ok(cx.undefined().upcast())
    }

    method strokeRect(mut cx){
      let mut this = cx.this();
      let nums = float_args(&mut cx, 0..4)?;
      if let [x, y, w, h] = nums.as_slice() {
        let rect = Rect::from_xywh(*x, *y, *w, *h);
        cx.borrow_mut(&mut this, |mut this| {
          let paint = this.paint_for_stroke();
          this.draw_rect(&rect, &paint);
        })
      }
      Ok(cx.undefined().upcast())
    }

    method clearRect(mut cx){
      let mut this = cx.this();
      let nums = float_args(&mut cx, 0..4)?;
      if let [x, y, w, h] = nums.as_slice() {
        let rect = Rect::from_xywh(*x, *y, *w, *h);
        cx.borrow_mut(&mut this, |mut this| {
          this.clear_rect(&rect);
        })
      }
      Ok(cx.undefined().upcast())
    }

    //
    // Imagery
    //
    // implemented in js:
    // - createImageData

    method getImageData(mut cx){
      let mut this = cx.this();
      let x = float_arg(&mut cx, 0, "x")? as i32;
      let y = float_arg(&mut cx, 1, "y")? as i32;
      let width = float_arg(&mut cx, 2, "width")? as i32;
      let height = float_arg(&mut cx, 3, "height")? as i32;

      let info = ImageInfo::new((width, height), ColorType::RGBA8888, AlphaType::Unpremul, None);
      let mut buffer = JsBuffer::new(&mut cx, 4 * (width * height) as u32)?;

      cx.borrow_mut(&mut buffer, |buf_data| {
        let mut buf_slice = buf_data.as_mut_slice();
        let row_bytes = (width * 4) as usize;
        cx.borrow_mut(&mut this, |mut this|
          if let Some(surface) = &mut this.surface{
            surface.read_pixels(&info, &mut buf_slice, row_bytes, (x, y));
          }
        )
      });

      let args = vec![cx.number(width), cx.number(height)];
      let img_data = JsImageData::new(&mut cx, args)?;
      let attr = cx.string("data");
      img_data.set(&mut cx, attr, buffer)?;

      Ok(img_data.upcast())
    }

    method putImageData(mut cx){
      let mut this = cx.this();
      let img_data = cx.argument::<JsImageData>(0)?;
      let info = cx.borrow(&img_data, |img_data| img_data.get_info() );

      // determine geometry
      let x = float_arg(&mut cx, 1, "x")?;
      let y = float_arg(&mut cx, 2, "y")?;
      let dirty = opt_float_args(&mut cx, 3..7);
      if !dirty.is_empty() && dirty.len() != 4 {
        return cx.throw_type_error("expected either 2 or 6 numbers")
      }
      let (width, height) = (info.width() as f32, info.height() as f32);
      let (src, dst) = match dirty.as_slice(){
        [dx, dy, dw, dh] => (
          Rect::from_xywh(*dx, *dy, *dw, *dh),
          Rect::from_xywh(*dx + x, *dy + y, *dw, *dh) ),
        _ => (
          Rect::from_xywh(0.0, 0.0, width, height),
          Rect::from_xywh(x, y, width, height)
      )};

      // convert buffer contents to image
      let mut buf = img_data.get(&mut cx, "data")?.downcast::<JsBuffer>().or_throw(&mut cx)?;
      let bmp_data = cx.borrow(&buf, |buf_data| Data::new_copy(&buf_data.as_slice()) );
      let row_size = info.width() as usize * 4;
      let image = Image::from_raster_data(&info, bmp_data, row_size);

      // draw to the canvas without any shaders, effects, transforms, etc.
      cx.borrow_mut(&mut this, |mut this| this.blit_image(&image, &src, &dst) );

      Ok(cx.undefined().upcast())
    }

    method drawImage(mut cx){
      let mut this = cx.this();
      let img = cx.argument::<JsImage>(0)?;
      let argc = cx.len() as usize;
      let nums = float_args(&mut cx, 1..argc)?;
      let dims = cx.borrow(&img, |img| {
        match &img.image {
          Some(image) => Some((image.width(), image.height())),
          None => None
        }
      });

      let (width, height) = match dims{
        Some((w,h)) => (w as f32, h as f32),
        None => return cx.throw_error("Cannot draw incomplete image (has it finished loading?)")
      };

      let (src, dst) = match nums.len() {
        2 => ( Rect::from_xywh(0.0, 0.0, width, height),
               Rect::from_xywh(nums[0], nums[1], width, height) ),
        4 => ( Rect::from_xywh(0.0, 0.0, width, height),
               Rect::from_xywh(nums[0], nums[1], nums[2], nums[3]) ),
        8 => ( Rect::from_xywh(nums[0], nums[1], nums[2], nums[3]),
               Rect::from_xywh(nums[4], nums[5], nums[6], nums[7]) ),
        _ => return cx.throw_error(format!("Expected 2, 4, or 8 coordinates (got {})", nums.len()))
      };

      cx.borrow_mut(&mut this, |mut this| {
        cx.borrow(&img, |img| {
          this.draw_image(&img.image, &src, &dst);
        });
      });

      Ok(cx.undefined().upcast())
    }

    //
    // Typography
    //
    method measureText(mut cx){
      let this = cx.this();
      unimplemented!();
      // Ok(cx.undefined().upcast())
    }
    method strokeText(mut cx){
      let this = cx.this();
      unimplemented!();
      // Ok(cx.undefined().upcast())
    }
    method fillText(mut cx){
      let this = cx.this();
      unimplemented!();
      // Ok(cx.undefined().upcast())
    }

    //
    // Shaders
    //
    // implemented in js:
    // - createLinearGradient
    // - createRadialGradient
    // - createPattern

    //
    // Output
    //

    method toBuffer(mut cx){
      let mut this = cx.this();
      let raster:Option<Data> = cx.borrow_mut(&mut this, |mut this|
        match &mut this.surface{
          Some(surface) => {
            let img = surface.image_snapshot();
            let data = img.encode_to_data(EncodedImageFormat::PNG).unwrap();
            Some(data)
          },
          None => None
        }
      );

      match raster{
        Some(data) => {
          let mut buffer = JsBuffer::new(&mut cx, data.len() as u32)?;
          cx.borrow_mut(&mut buffer, |buf_data| {
            buf_data.as_mut_slice().copy_from_slice(&data);
          });
          Ok(buffer.upcast())
        },
        None => Ok(cx.undefined().upcast())
      }
    }

    /* ---------------------------------------------------------------------- *
     |                              PROPERTIES                                |
     * ---------------------------------------------------------------------- */

     method get_canvas(mut cx){
      let this = cx.this();
      unimplemented!();
      // Ok(cx.undefined().upcast())
    }

    //
    // Geometry
    //

    method get_currentTransform(mut cx){
      let mut this = cx.this();
      let matrix = cx.borrow_mut(&mut this, |mut this| this.ctm() );
      matrix_to_array(&mut cx, &matrix)
    }

    method set_currentTransform(mut cx){
      let mut this = cx.this();
      let matrix = matrix_arg(&mut cx, 0)?;
      cx.borrow_mut(&mut this, |mut this|
        this.with_matrix(|ctm| ctm.reset().pre_concat(&matrix) )
      );
      Ok(cx.undefined().upcast())
    }

    //
    // Color
    //

    method get_fillStyle(mut cx){
      let this = cx.this();

      match cx.borrow(&this, |this| this.state.fill_style.clone() ){
        Dye::Gradient(gradient) => fetch_ref(&mut cx, "fillShader"),
        Dye::Pattern(pattern) => fetch_ref(&mut cx, "fillShader"),
        Dye::Color(color) => color_to_rgba(&mut cx, &color)
      }
    }

    method set_fillStyle(mut cx){
      let mut this = cx.this();

      let dye = match cx.argument::<JsValue>(0)?{
        arg if arg.is_a::<JsCanvasGradient>() => {
          stash_ref(&mut cx, "fillShader", arg)?;
          let gradient = cx.argument::<JsCanvasGradient>(0)?;
          cx.borrow(&gradient, |gradient| Dye::Gradient(gradient.clone()) )
        },
        arg if arg.is_a::<JsCanvasPattern>() => {
          stash_ref(&mut cx, "fillShader", arg)?;
          let pattern = cx.argument::<JsCanvasPattern>(0)?;
          cx.borrow(&pattern, |pattern| Dye::Pattern(pattern.clone()) )
        },
        _ => {
          let color = color_args(&mut cx, 0..4, "fillStyle")?;
          Dye::Color(color)
        }
      };

      cx.borrow_mut(&mut this, |mut this|  this.state.fill_style = dye );

      Ok(cx.undefined().upcast())
    }

    method get_strokeStyle(mut cx){
      let this = cx.this();

      match cx.borrow(&this, |this| this.state.stroke_style.clone() ){
        Dye::Gradient(gradient) => fetch_ref(&mut cx, "strokeShader"),
        Dye::Pattern(pattern) => fetch_ref(&mut cx, "strokeShader"),
        Dye::Color(color) => color_to_rgba(&mut cx, &color)
      }
    }

    method set_strokeStyle(mut cx){
      let mut this = cx.this();

      let dye = match cx.argument::<JsValue>(0)?{
        arg if arg.is_a::<JsCanvasGradient>() => {
          stash_ref(&mut cx, "strokeShader", arg)?;
          let gradient = cx.argument::<JsCanvasGradient>(0)?;
          cx.borrow(&gradient, |gradient| Dye::Gradient(gradient.clone()) )
        },
        arg if arg.is_a::<JsCanvasPattern>() => {
          stash_ref(&mut cx, "strokeShader", arg)?;
          let pattern = cx.argument::<JsCanvasPattern>(0)?;
          cx.borrow(&pattern, |pattern| Dye::Pattern(pattern.clone()) )
        },
        _ => {
          let color = color_args(&mut cx, 0..4, "strokeStyle")?;
          Dye::Color(color)
        }
      };

      cx.borrow_mut(&mut this, |mut this|  this.state.stroke_style = dye );

      Ok(cx.undefined().upcast())
    }

    method get_filter(mut cx){
      let this = cx.this();
      unimplemented!();
      // Ok(cx.undefined().upcast())
    }

    method set_filter(mut cx){
      let mut this = cx.this();
      unimplemented!();
      // Ok(cx.undefined().upcast())
    }

    //
    // Typography
    //

    method get_font(mut cx){
      let this = cx.this();
      unimplemented!();
      // Ok(cx.undefined().upcast())
    }

    method set_font(mut cx){
      let mut this = cx.this();
      unimplemented!();
      // Ok(cx.undefined().upcast())
    }

    method get_direction(mut cx){
      let this = cx.this();
      let name = cx.borrow(&this, |this|
        match this.state.text_ltr{ true => "ltr", false => "rtl" }.to_string()
      );
      Ok(cx.string(name).upcast())
    }

    method set_direction(mut cx){
      let mut this = cx.this();
      let name = string_arg(&mut cx, 0, "direction")?;
      if name=="ltr" || name=="rtl"{
        cx.borrow_mut(&mut this, |mut this| this.state.text_ltr = name=="ltr" );
      }
      Ok(cx.undefined().upcast())
    }

    method get_textAlign(mut cx){
      let this = cx.this();
      let mode = cx.borrow(&this, |this| this.state.text_align );
      let name = from_text_align(mode);
      Ok(cx.string(name).upcast())

    }

    method set_textAlign(mut cx){
      let mut this = cx.this();
      let name = string_arg(&mut cx, 0, "textAlign")?;
      if let Some(mode) = to_text_align(&name){
        cx.borrow_mut(&mut this, |mut this| this.state.text_align = mode );
      }
      Ok(cx.undefined().upcast())
    }

    method get_textBaseline(mut cx){
      let this = cx.this();
      let mode = cx.borrow(&this, |this| this.state.text_baseline );
      let name = from_text_baseline(mode);
      Ok(cx.string(name).upcast())
    }

    method set_textBaseline(mut cx){
      let mut this = cx.this();
      let name = string_arg(&mut cx, 0, "textBaseline")?;
      if let Some(mode) = to_text_baseline(&name){
        cx.borrow_mut(&mut this, |mut this| this.state.text_baseline = mode );
      }
      Ok(cx.undefined().upcast())
    }

    //
    // Compositing
    //

    method get_globalAlpha(mut cx){
      let this = cx.this();
      let num = cx.borrow(&this, |this| this.state.global_alpha );
      Ok(cx.number(num as f64).upcast())
    }

    method set_globalAlpha(mut cx){
      let mut this = cx.this();
      let num = float_arg(&mut cx, 0, "globalAlpha")?;
      if num <= 1.0 && num >= 0.0{
        cx.borrow_mut(&mut this, |mut this| this.state.global_alpha = num );
      }
      Ok(cx.undefined().upcast())
    }

    method get_globalCompositeOperation(mut cx){
      let this = cx.this();
      let mode = cx.borrow(&this, |this| this.state.global_composite_operation );
      let name = from_blend_mode(mode);
      Ok(cx.string(name).upcast())
    }

    method set_globalCompositeOperation(mut cx){
      let mut this = cx.this();
      let name = string_arg(&mut cx, 0, "globalCompositeOperation")?;
      if let Some(mode) = to_blend_mode(&name){
        cx.borrow_mut(&mut this, |mut this| this.state.global_composite_operation = mode );
      }
      Ok(cx.undefined().upcast())
    }

    method get_imageSmoothingEnabled(mut cx){
      let this = cx.this();
      let flag = cx.borrow(&this, |this| this.state.image_smoothing_enabled );
      Ok(cx.boolean(flag).upcast())
    }

    method set_imageSmoothingEnabled(mut cx){
      let mut this = cx.this();
      let flag = bool_arg(&mut cx, 0, "imageSmoothingEnabled")?;
      cx.borrow_mut(&mut this, |mut this| {
        this.state.image_smoothing_enabled = flag;
        this.update_image_quality();
      });
      Ok(cx.undefined().upcast())
    }

    method get_imageSmoothingQuality(mut cx){
      let this = cx.this();
      let mode = cx.borrow(&this, |this| this.state.image_filter_quality );
      let name = from_filter_quality(mode);
      Ok(cx.string(name).upcast())
    }

    method set_imageSmoothingQuality(mut cx){
      let mut this = cx.this();
      let name = string_arg(&mut cx, 0, "imageSmoothingQuality")?;
      if let Some(mode) = to_filter_quality(&name){
        cx.borrow_mut(&mut this, |mut this|{
          this.state.image_filter_quality = mode;
          this.update_image_quality();
        });
      }
      Ok(cx.undefined().upcast())
    }

    //
    // Shadow Effects
    //

    method get_shadowBlur(mut cx){
      let this = cx.this();
      let num = cx.borrow(&this, |this| this.state.shadow_blur );
      Ok(cx.number(num as f64).upcast())
    }

    method set_shadowBlur(mut cx){
      let mut this = cx.this();
      let num = float_arg(&mut cx, 0, "shadowBlur")?;
      if num >= 0.0{
        cx.borrow_mut(&mut this, |mut this| this.state.shadow_blur = num );
      }
      Ok(cx.undefined().upcast())
    }

    method get_shadowColor(mut cx){
      let this = cx.this();
      let shadow_color = cx.borrow(&this, |this| this.state.shadow_color );
      color_to_rgba(&mut cx, &shadow_color)
    }

    method set_shadowColor(mut cx){
      let mut this = cx.this();
      let color = color_args(&mut cx, 0..4, "shadowColor")?;
      cx.borrow_mut(&mut this, |mut this| { this.state.shadow_color = color; });
      Ok(cx.undefined().upcast())
    }

    method get_shadowOffsetX(mut cx){
      let this = cx.this();
      let num = cx.borrow(&this, |this| this.state.shadow_offset.x );
      Ok(cx.number(num as f64).upcast())
    }

    method set_shadowOffsetX(mut cx){
      let mut this = cx.this();
      let num = float_arg(&mut cx, 0, "shadowOffsetX")?;
      cx.borrow_mut(&mut this, |mut this| this.state.shadow_offset.x = num );
      Ok(cx.undefined().upcast())
    }

    method get_shadowOffsetY(mut cx){
      let this = cx.this();
      let num = cx.borrow(&this, |this| this.state.shadow_offset.y );
      Ok(cx.number(num as f64).upcast())
    }

    method set_shadowOffsetY(mut cx){
      let mut this = cx.this();
      let num = float_arg(&mut cx, 0, "shadowOffsetY")?;
      cx.borrow_mut(&mut this, |mut this| this.state.shadow_offset.y = num );
      Ok(cx.undefined().upcast())
    }

    //
    // Line Style
    //

    method get_lineCap(mut cx){
      let this = cx.this();
      let mode = cx.borrow(&this, |this| this.state.paint.stroke_cap() );
      let name = from_stroke_cap(mode);
      Ok(cx.string(name).upcast())
    }

    method set_lineCap(mut cx){
      let mut this = cx.this();
      let name = string_arg(&mut cx, 0, "lineCap")?;
      if let Some(mode) = to_stroke_cap(&name){
        cx.borrow_mut(&mut this, |mut this|{ this.state.paint.set_stroke_cap(mode); });
      }
      Ok(cx.undefined().upcast())
    }

    method get_lineDashOffset(mut cx){
      let this = cx.this();
      let num = cx.borrow(&this, |this| this.state.line_dash_offset );
      Ok(cx.number(num as f64).upcast())
    }

    method set_lineDashOffset(mut cx){
      let mut this = cx.this();
      let num = float_arg(&mut cx, 0, "lineDashOffset")?;
      cx.borrow_mut(&mut this, |mut this| this.state.line_dash_offset = num );
      Ok(cx.undefined().upcast())
    }

    method get_lineJoin(mut cx){
      let this = cx.this();
      let mode = cx.borrow(&this, |this| this.state.paint.stroke_join() );
      let name = from_stroke_join(mode);
      Ok(cx.string(name).upcast())
    }

    method set_lineJoin(mut cx){
      let mut this = cx.this();
      let name = string_arg(&mut cx, 0, "lineJoin")?;
      if let Some(mode) = to_stroke_join(&name){
        cx.borrow_mut(&mut this, |mut this|{ this.state.paint.set_stroke_join(mode); });
      }
      Ok(cx.undefined().upcast())
    }

    method get_lineWidth(mut cx){
      let this = cx.this();
      let num = cx.borrow(&this, |this| this.state.paint.stroke_width() );
      Ok(cx.number(num as f64).upcast())
    }

    method set_lineWidth(mut cx){
      let mut this = cx.this();
      let num = float_arg(&mut cx, 0, "lineWidth")?;
      cx.borrow_mut(&mut this, |mut this|{
        this.state.paint.set_stroke_width(num);
        this.state.stroke_width = num;
      });
      Ok(cx.undefined().upcast())
    }

    method get_miterLimit(mut cx){
      let this = cx.this();
      let num = cx.borrow(&this, |this| this.state.paint.stroke_miter() );
      Ok(cx.number(num as f64).upcast())
    }

    method set_miterLimit(mut cx){
      let mut this = cx.this();
      let num = float_arg(&mut cx, 0, "miterLimit")?;
      cx.borrow_mut(&mut this, |mut this|{ this.state.paint.set_stroke_miter(num); });
      Ok(cx.undefined().upcast())
    }

  }
}