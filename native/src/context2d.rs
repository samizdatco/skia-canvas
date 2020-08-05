#![allow(unused_mut)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]
use std::f32::consts::PI;
use neon::prelude::*;
use skia_safe::{Surface, Canvas, Paint, PaintStyle, BlendMode, FilterQuality, MaskFilter, BlurStyle};
use skia_safe::{Path, Matrix, PathDirection, Rect, Point, scalar, path::{AddPathMode, FillType}};
use skia_safe::{Data, Image, EncodedImageFormat, Font, Color, Color4f, Shader, dash_path_effect};
use skia_safe::{utils::text_utils::Align};

use crate::utils::*;
use crate::path2d::{Path2D, JsPath2D};
use crate::gradient::{CanvasGradient, JsCanvasGradient};

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
  transform: Matrix,

  font_string: String,
  text_ltr: bool,
  text_align: Align,
  text_baseline: Baseline,

  stroke_style: Dye,
  fill_style: Dye,
  shadow_blur: scalar,
  shadow_color: Color,
  shadow_offset: Point,

  global_alpha: scalar,
  stroke_width: scalar,
  line_dash_offset: scalar,
  line_dash_list: Vec<scalar>,

  global_composite_operation: BlendMode,
  image_filter_quality: FilterQuality,
  image_smoothing_enabled: bool,
}

#[derive(Clone)]
pub enum Dye{
  Color(Color),
  Shader{shader:Shader}
}

impl Context2D{
  pub fn to_local_coordinates(&self, x: f32, y: f32) -> Point{
    match self.state.transform.clone().invert(){
      Some(inverse) => inverse.map_point((x, y)),
      None => (x, y).into()
    }
  }

  pub fn draw_path(&mut self, paint: &Paint){
    let path = &mut self.path;

    // draw shadow if applicable
    if let Some(shadow_paint) = self.paint_for_shadow(&paint){
      if let Some(surface) = &mut self.surface{
        let canvas = surface.canvas();
        canvas.save();

        let inverted = self.state.transform.clone().invert().unwrap();
        let nudge = Matrix::new_trans(self.state.shadow_offset);
        canvas.concat(&inverted);
        canvas.concat(&nudge);
        canvas.concat(&self.state.transform);
        canvas.draw_path(&self.path, &shadow_paint);

        canvas.restore();
      }
    }

    // then draw the actual path
    if let Some(surface) = &mut self.surface{
      surface.canvas().draw_path(&self.path, &paint);
    }
  }

  pub fn draw_rect(&mut self, rect:&Rect, paint: &Paint){
    let path = &mut self.path;

    // draw shadow if applicable
    if let Some(shadow_paint) = self.paint_for_shadow(&paint){
      if let Some(surface) = &mut self.surface{
        let canvas = surface.canvas();
        canvas.save();

        let inverted = self.state.transform.clone().invert().unwrap();
        let nudge = Matrix::new_trans(self.state.shadow_offset);
        canvas.concat(&inverted);
        canvas.concat(&nudge);
        canvas.concat(&self.state.transform);
        canvas.draw_rect(&rect, &shadow_paint);

        canvas.restore();
      }
    }

    // then draw the actual rect
    if let Some(surface) = &mut self.surface{
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

  pub fn color_with_alpha(&self, src:&Color) -> Color{
    let mut color:Color4f = src.clone().into();
    color.a *= self.state.global_alpha;
    color.to_color()
  }

  pub fn paint_for_shadow(&self, base_paint:&Paint) -> Option<Paint>{
    let c = self.state.shadow_color;
    let shadow_color = self.color_with_alpha(&self.state.shadow_color);
    let State {shadow_blur, shadow_offset, ..} = self.state;
    if shadow_color.a() == 0 || shadow_blur == 0.0 || shadow_offset.is_zero(){
      return None
    }

    let mut shadow_paint = base_paint.clone();
    shadow_paint.set_color(shadow_color);
    let blur_filter = MaskFilter::blur(BlurStyle::Normal, shadow_blur/2.0, Some(false));
    shadow_paint.set_mask_filter(blur_filter);

    Some(shadow_paint)
  }

  pub fn paint_for_fill(&self) -> Paint{
    let mut paint = self.state.paint.clone();
    paint.set_style(PaintStyle::Fill);

    match &self.state.fill_style{
      Dye::Color(color) => { paint.set_color(self.color_with_alpha(&color)); },
      Dye::Shader{shader,..} => {paint.set_shader(Some(shader.clone()));}
    }

    paint
  }

  pub fn paint_for_stroke(&self) -> Paint{
    let mut paint = self.state.paint.clone();
    paint.set_style(PaintStyle::Stroke);
    paint.set_stroke_width(self.state.stroke_width);

    match &self.state.stroke_style{
      Dye::Color(color) => { paint.set_color(self.color_with_alpha(&color)); },
      Dye::Shader{shader,..} => {paint.set_shader(Some(shader.clone()));}
    }

    if !self.state.line_dash_list.is_empty() {
      let dash = dash_path_effect::new(&self.state.line_dash_list, self.state.line_dash_offset);
      paint.set_path_effect(dash);
    }

    paint
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

      Ok( Context2D{
        surface: None,
        path: Path::new(),
        font: None,
        state_stack: vec![],
        state: State {
          transform: Matrix::new_identity(),

          font_string: "10px monospace".to_string(),
          text_ltr: true,
          text_align: Align::Left,
          text_baseline: Baseline::Alphabetic,

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
     |                              PROPERTIES                                |
     * ---------------------------------------------------------------------- */

    method get_canvas(mut cx){
      let this = cx.this();
      Ok(cx.undefined().upcast())
    }

    //
    // Geometry
    //

    method get_currentTransform(mut cx){
      let this = cx.this();
      let array = JsArray::new(&mut cx, 9 as u32);
      let mat_vec:Vec<f64> = cx.borrow(&this, |this|
        (0..9).map(|i| this.state.transform[i as usize] as f64).collect()
      );
      for (i, term) in mat_vec.iter().enumerate() {
        let num = cx.number(*term);
        array.set(&mut cx, i as u32, num).unwrap();
      }

      Ok(array.upcast())
    }

    method set_currentTransform(mut cx){
      let mut this = cx.this();
      let arg = cx.argument::<JsArray>(0)?.to_vec(&mut cx)?;
      let matrix = matrix_in(&mut cx, &arg)?;
      cx.borrow_mut(&mut this, |mut this| this.state.transform = matrix );
      Ok(cx.undefined().upcast())
    }

    //
    // Color
    //

    method get_fillStyle(mut cx){
      let this = cx.this();

      match cx.borrow(&this, |this| this.state.fill_style.clone() ){
        Dye::Color(color) => {
          let color:Color4f = color.into();
          let rgba = JsArray::new(&mut cx, 4);
          for (i, c) in color.as_array().iter().enumerate(){
            let c = cx.number(*c as f64);
            rgba.set(&mut cx, i as u32, c)?;
          }
          Ok(rgba.upcast())
        },
        Dye::Shader{shader} => {
          println!("Unimplemented: return ref to CanvasGradient");
          Ok(cx.empty_object().upcast())
        }
      }
    }

    method set_fillStyle(mut cx){
      let mut this = cx.this();

      if cx.argument::<JsValue>(0)?.is_a::<JsCanvasGradient>(){
        let gradient = cx.argument::<JsCanvasGradient>(0)?;
        if let Some(shader) = cx.borrow(&gradient, |gradient| gradient.shader()){
          cx.borrow_mut(&mut this, |mut this| {
            this.state.fill_style = Dye::Shader{shader};
          });
        }
      }else{
        let color = color_args(&mut cx, 0..4, "fillStyle")?;
        cx.borrow_mut(&mut this, |mut this| { this.state.fill_style = Dye::Color(color); });
      }

      Ok(cx.undefined().upcast())
    }

    method get_strokeStyle(mut cx){
      let this = cx.this();

      match cx.borrow(&this, |this| this.state.stroke_style.clone() ){
        Dye::Color(color) => {
          let color:Color4f = color.into();
          let rgba = JsArray::new(&mut cx, 4);
          for (i, c) in color.as_array().iter().enumerate(){
            let c = cx.number(*c as f64);
            rgba.set(&mut cx, i as u32, c)?;
          }
          Ok(rgba.upcast())
        },
        Dye::Shader{shader} => {
          println!("Unimplemented: return ref to CanvasGradient");
          Ok(cx.empty_object().upcast())
        }
      }

    }

    method set_strokeStyle(mut cx){
      let mut this = cx.this();

      if cx.argument::<JsValue>(0)?.is_a::<JsCanvasGradient>(){
        let gradient = cx.argument::<JsCanvasGradient>(0)?;
        if let Some(shader) = cx.borrow(&gradient, |gradient| gradient.shader()){
          cx.borrow_mut(&mut this, |mut this| {
            this.state.stroke_style = Dye::Shader{shader};
          });
        }
      }else{
        let color = color_args(&mut cx, 0..4, "fillStyle")?;
        cx.borrow_mut(&mut this, |mut this| { this.state.stroke_style = Dye::Color(color); });
      }

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
      cx.borrow_mut(&mut this, |mut this| this.state.image_smoothing_enabled = flag );
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
        cx.borrow_mut(&mut this, |mut this| this.state.image_filter_quality = mode );
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
      cx.borrow_mut(&mut this, |mut this| this.state.shadow_blur = num );
      Ok(cx.undefined().upcast())
    }

    method get_shadowColor(mut cx){
      let this = cx.this();
      let color:Color4f = cx.borrow(&this, |this| this.state.shadow_color.into() );
      let rgba = JsArray::new(&mut cx, 4);
      for (i, c) in color.as_array().iter().enumerate(){
        let c = cx.number(*c as f64);
        rgba.set(&mut cx, i as u32, c)?;
      }
      Ok(rgba.upcast())
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


    /* ---------------------------------------------------------------------- *
     |                                METHODS                                 |
     * ---------------------------------------------------------------------- */

    //
    // State
    //

    method save(mut cx){
      let mut this = cx.this();
      cx.borrow_mut(&mut this, |mut this| {
        let new_state = this.state.clone();
        this.state_stack.push(new_state);
      });
      Ok(cx.undefined().upcast())
    }

    method restore(mut cx){
      let mut this = cx.this();
      let success = cx.borrow_mut(&mut this, |mut this| {
        match this.state_stack.pop(){
          Some(old_state) =>{ this.state = old_state; true},
          None => false
        }
      });
      if !success{ return cx.throw_error("no saved state to restore") }
      Ok(cx.undefined().upcast())
    }

    method getLineDash(mut cx){
      let mut this = cx.this();
      let dashes = cx.borrow(&this, |this| this.state.line_dash_list.clone());
      let array = JsArray::new(&mut cx, dashes.len() as u32);
      for (i, interval) in dashes.iter().enumerate() {
        let num = cx.number(*interval);
        array.set(&mut cx, i as u32, num).unwrap();
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

    method clip(mut cx){
      let this = cx.this();
      unimplemented!();
      // Ok(cx.undefined().upcast())
    }

    //
    // Matrix
    //
    // Implemented in js:
    // - getTransform
    // - setTransform

    method resetTransform(mut cx){
      let mut this = cx.this();
      cx.borrow_mut(&mut this, |mut this| {
        this.state.transform = Matrix::new_identity();
      });
      Ok(cx.undefined().upcast())
    }

    method rotate(mut cx){
      let mut this = cx.this();
      let radians = float_arg(&mut cx, 0, "angle")?;
      let degrees = radians * PI / 180.0;
      cx.borrow_mut(&mut this, |mut this| {
        this.state.transform.pre_rotate(degrees, None);
      });
      Ok(cx.undefined().upcast())
    }

    method scale(mut cx){
      let mut this = cx.this();
      let x_scale = float_arg(&mut cx, 0, "xScale")?;
      let y_scale = float_arg(&mut cx, 0, "yScale")?;
      cx.borrow_mut(&mut this, |mut this| {
        this.state.transform.pre_scale((x_scale, y_scale), None);
      });
      Ok(cx.undefined().upcast())
    }

    method transform(mut cx){
      let mut this = cx.this();
      let arg = cx.argument::<JsArray>(0)?.to_vec(&mut cx)?;
      let matrix = matrix_in(&mut cx, &arg)?;
      cx.borrow_mut(&mut this, |mut this| {
        this.state.transform.pre_concat(&matrix);
      });
      Ok(cx.undefined().upcast())
    }

    method translate(mut cx){
      let mut this = cx.this();
      let dx = float_arg(&mut cx, 0, "deltaX")?;
      let dy = float_arg(&mut cx, 0, "deltaY")?;
      cx.borrow_mut(&mut this, |mut this| {
        this.state.transform.pre_translate((dx, dy));
      });
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

    method arc(mut cx){
      let mut this = cx.this();
      let nums = float_args(&mut cx, 0..5)?;
      let ccw = bool_arg(&mut cx, 5, "isCCW")?;

      if let [x, y, radius, start_angle, end_angle] = nums.as_slice(){
        cx.borrow_mut(&mut this, |mut this| {
          let mut arc = Path2D::new();
          arc.add_ellipse((*x, *y), (*radius, *radius), 0.0, *start_angle, *end_angle, ccw);
          this.path.add_path(&arc.path, (0,0), AddPathMode::Append);
        });
      }
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

    method moveTo(mut cx){
      let mut this = cx.this();
      let x = float_arg(&mut cx, 0, "x")?;
      let y = float_arg(&mut cx, 1, "y")?;
      cx.borrow_mut(&mut this, |mut this| {
        this.path.move_to((x, y));
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

      let point = cx.borrow(&this, |this| this.to_local_coordinates(x, y) );
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
      let point = cx.borrow(&this, |this| this.to_local_coordinates(x, y) );

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

    //
    // Drawing
    //
    method fill(mut cx){
      let mut this = cx.this();
      let mut shift = 0;
      if let Some(arg) = cx.argument_opt(0){
        if let Ok(arg) = arg.downcast::<JsPath2D>(){
          cx.borrow_mut(&mut this, |mut this| {
            cx.borrow(&arg, |arg| this.path = arg.path.clone());
          });
          shift += 1;
        }
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
      if let Some(arg) = cx.argument_opt(0){
        if let Ok(arg) = arg.downcast::<JsPath2D>(){
          cx.borrow_mut(&mut this, |mut this| {
            cx.borrow(&arg, |arg| this.path = arg.path.clone());
          });
        }
      }

      cx.borrow_mut(&mut this, |mut this| {
        let paint = this.paint_for_stroke();
        this.draw_path(&paint);
      });

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

    //
    // Imagery
    //

    method createImageData(mut cx){
      let this = cx.this();
      unimplemented!();
      // Ok(cx.undefined().upcast())
    }
    method getImageData(mut cx){
      let this = cx.this();
      unimplemented!();
      // Ok(cx.undefined().upcast())
    }
    method putImageData(mut cx){
      let this = cx.this();
      unimplemented!();
      // Ok(cx.undefined().upcast())
    }
    method drawImage(mut cx){
      let this = cx.this();
      unimplemented!();
      // Ok(cx.undefined().upcast())
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

    //
    // Image Shader / 2D Path Effect
    //
    method createPattern(mut cx){
      let this = cx.this();
      unimplemented!();
      // Ok(cx.undefined().upcast())
    }

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

  }
}