use std::f32::consts::PI;
use neon::prelude::*;
use skia_safe::{Path, Matrix, Rect, PathDirection};
use skia_safe::path::AddPathMode::Append;
use skia_safe::textlayout::{TextDirection};
use skia_safe::PaintStyle::{Fill, Stroke};

use super::{Context2D, Dye};
use crate::canvas::{JsCanvas, canvas_context};
use crate::path::{Path2D, JsPath2D};
use crate::image::{JsImage, JsImageData};
use crate::typography::*;
use crate::utils::*;

//
// The js interface for the Context2D struct
//

declare_types! {
  pub class JsContext2D for Context2D {
    init(mut cx) {
      if cx.len() == 3 {
        let dims = float_args(&mut cx, 0..2)?;
        let fonts = cx.argument::<JsFontLibrary>(2)?;

        return cx.borrow(&fonts, |fonts|{
          let bounds = Rect::from_wh(dims[0], dims[1]);
          Ok(Context2D::new(bounds, &fonts.library))
        })
      }

      // direct use of this is nonstandard in any case, so we can at least
      // pretend not to exist
      cx.throw_type_error("function is not a constructor")
    }

    //
    // Grid State (see js for getTransform & setTransform)
    //

    method save(mut cx){
      let mut this = cx.this();
      cx.borrow_mut(&mut this, |mut this| this.push() );
      Ok(cx.undefined().upcast())
    }

    method restore(mut cx){
      let mut this = cx.this();
      cx.borrow_mut(&mut this, |mut this| this.pop() );
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

    method transform(mut cx){
      let mut this = cx.this();
      let t = float_args(&mut cx, 0..6)?;
      let matrix = Matrix::new_all(t[0], t[2], t[4], t[1], t[3], t[5], 0.0, 0.0, 1.0);

      cx.borrow_mut(&mut this, |mut this| {
        this.with_matrix(|ctm| ctm.pre_concat(&matrix) );
      });
      Ok(cx.undefined().upcast())
    }

    method translate(mut cx){
      let mut this = cx.this();
      let dx = float_arg(&mut cx, 0, "deltaX")?;
      let dy = float_arg(&mut cx, 1, "deltaY")?;
      cx.borrow_mut(&mut this, |mut this| {
        this.with_matrix(|ctm| ctm.pre_translate((dx, dy)) );
      });
      Ok(cx.undefined().upcast())
    }

    method scale(mut cx){
      let mut this = cx.this();
      let x_scale = float_arg(&mut cx, 0, "xScale")?;
      let y_scale = float_arg(&mut cx, 1, "yScale")?;
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

    // -- ctm property --------------------------------------------------------------------

    method get_currentTransform(mut cx){
      let this = cx.this();
      let matrix = cx.borrow(&this, |this| this.state.matrix );
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
    // Bézier Paths
    //

    method beginPath(mut cx){
      let mut this = cx.this();
      cx.borrow_mut(&mut this, |mut this| {
        this.path = Path::new();
      });
      Ok(cx.undefined().upcast())
    }

    // -- primitives --------------------------------------------------------------------

    method rect(mut cx){
      let mut this = cx.this();
      let nums = float_args(&mut cx, 0..4)?;
      if let [x, y, w, h] = nums.as_slice(){
        let rect = Rect::from_xywh(*x, *y, *w, *h);
        cx.borrow_mut(&mut this, |mut this| {
          let matrix = this.state.matrix;
          let mut rect_path = Path::new();
          rect_path.add_rect(&rect, Some((PathDirection::CW, 0)));
          this.path.add_path(&rect_path.with_transform(&matrix), (0, 0), Append);
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
          let matrix = this.state.matrix;
          let mut arc = Path2D::new();
          arc.add_ellipse((*x, *y), (*radius, *radius), 0.0, *start_angle, *end_angle, ccw);
          this.path.add_path(&arc.path.with_transform(&matrix), (0,0), Append);
        });
      }
      Ok(cx.undefined().upcast())
    }

    method ellipse(mut cx){
      let mut this = cx.this();
      let nums = float_args(&mut cx, 0..7)?;
      let ccw = bool_arg_or(&mut cx, 7, false);

      if let [x, y, x_radius, y_radius, rotation, start_angle, end_angle] = nums.as_slice(){
        if *x_radius < 0.0 || *y_radius < 0.0 {
          return cx.throw_error("radii cannot be negative")
        }
        cx.borrow_mut(&mut this, |mut this| {
          let matrix = this.state.matrix;
          let mut arc = Path2D::new();
          arc.add_ellipse((*x, *y), (*x_radius, *y_radius), *rotation, *start_angle, *end_angle, ccw);
          this.path.add_path(&arc.path.with_transform(&matrix), (0,0), Append);
        });
      }

      Ok(cx.undefined().upcast())
    }

    // contour drawing ----------------------------------------------------------------------

    method moveTo(mut cx){
      let mut this = cx.this();
      let x = float_arg(&mut cx, 0, "x")?;
      let y = float_arg(&mut cx, 1, "y")?;
      cx.borrow_mut(&mut this, |mut this| {
        if let [dst] = this.map_points(&[x, y])[..1]{
          this.path.move_to(dst);
        }
      });
      Ok(cx.undefined().upcast())
    }

    method lineTo(mut cx){
      let mut this = cx.this();
      let x = float_arg(&mut cx, 0, "x")?;
      let y = float_arg(&mut cx, 1, "y")?;
      cx.borrow_mut(&mut this, |mut this| {
        if let [dst] = this.map_points(&[x, y])[..1]{
          if this.path.is_empty(){ this.path.move_to(dst); }
          this.path.line_to(dst);
        }
      });
      Ok(cx.undefined().upcast())
    }

    method arcTo(mut cx){
      let mut this = cx.this();
      let coords = float_args(&mut cx, 0..4)?;
      let radius = float_arg(&mut cx, 4, "radius")?;

      cx.borrow_mut(&mut this, |mut this| {
        if let [src, dst] = this.map_points(&coords)[..2]{
          if this.path.is_empty(){ this.path.move_to(src); }
          this.path.arc_to_tangent(src, dst, radius);
        }
      });

      Ok(cx.undefined().upcast())
    }

    method bezierCurveTo(mut cx){
      let mut this = cx.this();
      let coords = float_args(&mut cx, 0..6)?;
      cx.borrow_mut(&mut this, |mut this| {
        if let [cp1, cp2, dst] = this.map_points(&coords)[..3]{
          if this.path.is_empty(){ this.path.move_to(cp1); }
          this.path.cubic_to(cp1, cp2, dst);
        }
      });
      Ok(cx.undefined().upcast())
    }

    method quadraticCurveTo(mut cx){
      let mut this = cx.this();
      let coords = float_args(&mut cx, 0..4)?;

      cx.borrow_mut(&mut this, |mut this| {
        if let [cp, dst] = this.map_points(&coords)[..2]{
          if this.path.is_empty(){ this.path.move_to(cp); }
          this.path.quad_to(cp, dst);
        }
      });
      Ok(cx.undefined().upcast())
    }

    method closePath(mut cx){
      let mut this = cx.this();
      cx.borrow_mut(&mut this, |mut this| {
        this.path.close();
      });
      Ok(cx.undefined().upcast())
    }

    // hit testing ----------------------------------------------------------------------

    method isPointInPath(mut cx){
      let mut this = cx.this();
      let (mut container, shift) = match cx.argument::<JsValue>(0)?.is_a::<JsPath2D>(){
        true => (cx.argument(0)?, 1),
        false => (this, 0)
      };
      let x = float_arg(&mut cx, shift, "x")?;
      let y = float_arg(&mut cx, shift+1, "y")?;
      let rule = fill_rule_arg_or(&mut cx, shift+2, "nonzero")?;
      let is_in = cx.borrow_mut(&mut container, |mut obj| {
        cx.borrow_mut(&mut this, |mut this|
          this.hit_test_path(&mut obj.path, (x, y), Some(rule), Fill)
        )
      });
      Ok(cx.boolean(is_in).upcast())
    }

    method isPointInStroke(mut cx){
      let mut this = cx.this();
      let (mut container, shift) = match cx.argument::<JsValue>(0)?.is_a::<JsPath2D>(){
        true => (cx.argument(0)?, 1),
        false => (this, 0)
      };
      let x = float_arg(&mut cx, shift, "x")?;
      let y = float_arg(&mut cx, shift+1, "y")?;
      let is_in = cx.borrow_mut(&mut container, |mut obj| {
        cx.borrow_mut(&mut this, |mut this|
          this.hit_test_path(&mut obj.path, (x, y), None, Stroke)
        )
      });

      Ok(cx.boolean(is_in).upcast())
    }

    //
    // Fill & Stroke
    //

    method fill(mut cx){
      let mut this = cx.this();

      let mut shift = 0;
      if let Some(path) = path2d_arg_opt(&mut cx, 0){
        cx.borrow_mut(&mut this, |mut this| {
          this.path = path.with_transform(&this.state.matrix)
        });
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
        cx.borrow_mut(&mut this, |mut this| {
          this.path = path.with_transform(&this.state.matrix)
        });
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

    // fill & stoke properties -------------------------------------------------------------

    method get_fillStyle(mut cx){
      let this = cx.this();
      let dye = cx.borrow(&this, |this| this.state.fill_style.clone() );
      dye.value(&mut cx, Fill)
    }

    method set_fillStyle(mut cx){
      let mut this = cx.this();
      let arg = cx.argument::<JsValue>(0)?;
      if let Some(dye) = Dye::new(&mut cx, arg, Fill)? {
        cx.borrow_mut(&mut this, |mut this|{
          this.state.fill_style = dye;
          this.update_image_quality();
        });
      }else{
        eprintln!("Warning: Invalid fill style (expected a css color string, CanvasGradient, or CanvasPattern)");
      }

      Ok(cx.undefined().upcast())
    }

    method get_strokeStyle(mut cx){
      let this = cx.this();
      let dye = cx.borrow(&this, |this| this.state.stroke_style.clone() );
      dye.value(&mut cx, Stroke)
    }

    method set_strokeStyle(mut cx){
      let mut this = cx.this();
      let arg = cx.argument::<JsValue>(0)?;
      if let Some(dye) = Dye::new(&mut cx, arg, Stroke)? {
        cx.borrow_mut(&mut this, |mut this|{
          this.state.stroke_style = dye;
          this.update_image_quality();
        });
      }else{
        eprintln!("Warning: Invalid stroke style (expected a css color string, CanvasGradient, or CanvasPattern)");
      }

      Ok(cx.undefined().upcast())
    }

    //
    // Line Style
    //

    method getLineDash(mut cx){
      let this = cx.this();
      let dashes = cx.borrow(&this, |this| this.state.line_dash_list.clone());
      floats_to_array(&mut cx, &dashes)
    }

    method setLineDash(mut cx){
      let mut this = cx.this();
      let arg = cx.argument::<JsValue>(0)?;
      if arg.is_a::<JsArray>() {
        let list = cx.argument::<JsArray>(0)?.to_vec(&mut cx)?;
        let mut intervals = floats_in(&list).iter().cloned()
          .filter(|n| *n >= 0.0)
          .collect::<Vec<f32>>();

        if list.len() == intervals.len(){
          if intervals.len() % 2 == 1{
            intervals.append(&mut intervals.clone());
          }

          cx.borrow_mut(&mut this, |mut this| {
            this.state.line_dash_list = intervals
          });
        }
      }
      Ok(cx.undefined().upcast())
    }

    // line style properties  -----------------------------------------------------------

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
      if let Some(num) = opt_float_arg(&mut cx, 0){
        if num > 0.0 {
          cx.borrow_mut(&mut this, |mut this|{
            this.state.paint.set_stroke_width(num);
            this.state.stroke_width = num;
          });
        }
      }
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

    //
    // Imagery (see js for createImageData)
    //

    method drawImage(mut cx){
      let mut this = cx.this();
      let arg = cx.argument::<JsObject>(0)?;
      let canvas = arg.downcast::<JsCanvas>().ok();
      let image = arg.downcast::<JsImage>().ok();

      let dims = if let Some(canvas) = canvas{
        cx.borrow(&canvas, |canvas| Some(
          (canvas.width as i32, canvas.height as i32)
        ))
      }else if let Some(image) = image{
        cx.borrow(&image, |img| img.image.as_ref().map(|img|
          (img.width(), img.height())
        ))
      }else{
        return cx.throw_type_error("Expected an Image or a Canvas argument")
      };

      let (width, height) = match dims{
        Some((w,h)) => (w as f32, h as f32),
        None => return cx.throw_error("Cannot draw incomplete image (has it finished loading?)")
      };

      let argc = cx.len() as usize;
      let nums = float_args(&mut cx, 1..argc)?;
      let (src, dst) = match nums.len() {
        2 => ( Rect::from_xywh(0.0, 0.0, width, height),
               Rect::from_xywh(nums[0], nums[1], width, height) ),
        4 => ( Rect::from_xywh(0.0, 0.0, width, height),
               Rect::from_xywh(nums[0], nums[1], nums[2], nums[3]) ),
        8 => ( Rect::from_xywh(nums[0], nums[1], nums[2], nums[3]),
               Rect::from_xywh(nums[4], nums[5], nums[6], nums[7]) ),
        _ => return cx.throw_error(format!("Expected 2, 4, or 8 coordinates (got {})", nums.len()))
      };

      // shrink src to lie within the image bounds and adjust dst proportionately
      let (src, dst) = fit_bounds(width, height, src, dst);

      if let Some(img) = image {
        cx.borrow_mut(&mut this, |mut this| {
          cx.borrow(&img, |img| {
            this.draw_image(&img.image, &src, &dst);
          });
        });
      }else if let Some(canvas) = canvas {
        let pict = canvas_context(&mut cx, &canvas, |ctx| ctx.get_picture(None) )?;
        cx.borrow_mut(&mut this, |mut this| {
            this.draw_picture(&pict, &src, &dst);
        });
      }

      Ok(cx.undefined().upcast())
    }

    method _getImageData(mut cx){
      let mut this = cx.this();
      let x = float_arg(&mut cx, 0, "x")? as i32;
      let y = float_arg(&mut cx, 1, "y")? as i32;
      let width = float_arg(&mut cx, 2, "width")? as i32;
      let height = float_arg(&mut cx, 3, "height")? as i32;

      let buffer = JsBuffer::new(&mut cx, 4 * (width * height) as u32)?;
      cx.borrow(&buffer, |data| {
        cx.borrow_mut(&mut this, |mut this|{
          this.get_pixels(data.as_mut_slice(), (x, y), (width, height));
        })
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
      let mut dirty = opt_float_args(&mut cx, 3..7);
      if !dirty.is_empty() && dirty.len() != 4 {
        return cx.throw_type_error("expected either 2 or 6 numbers")
      }
      let (width, height) = (info.width() as f32, info.height() as f32);
      let (mut src, mut dst) = match dirty.as_mut_slice(){
        [dx, dy, dw, dh] => {
          if *dw < 0.0 { *dw *= -1.0; *dx -= *dw; }
          if *dh < 0.0 { *dh *= -1.0; *dy -= *dh; }
          (Rect::from_xywh(*dx, *dy, *dw, *dh), Rect::from_xywh(*dx + x, *dy + y, *dw, *dh))
        },
        _ => (
          Rect::from_xywh(0.0, 0.0, width, height),
          Rect::from_xywh(x, y, width, height)
      )};

      let buffer = img_data.get(&mut cx, "data")?.downcast_or_throw::<JsBuffer, _>(&mut cx)?;
      cx.borrow(&buffer, |data| {
        cx.borrow_mut(&mut this, |mut this|{
          this.blit_pixels(data.as_slice(), &info, &src, &dst);
        })
      });

      Ok(cx.undefined().upcast())
    }

    // -- image properties --------------------------------------------------------------

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
    // Typography
    //

    method _fillText(mut cx){
      let mut this = cx.this();
      let text = string_arg(&mut cx, 0, "text")?;
      let x = float_arg(&mut cx, 1, "x")?;
      let y = float_arg(&mut cx, 2, "y")?;
      let width = opt_float_arg(&mut cx, 3);

      if width.is_none() && cx.len() > 3 && !cx.argument::<JsValue>(3)?.is_a::<JsUndefined>(){
        // it's fine to include an ignored `undefined` but anything else is invalid
        return Ok(cx.undefined().upcast())
      }

      cx.borrow_mut(&mut this, |mut this|{
        let paint = this.paint_for_fill();
        this.draw_text(&text, x, y, width, paint);
      });

      Ok(cx.undefined().upcast())
    }

    method _strokeText(mut cx){
      let mut this = cx.this();
      let text = string_arg(&mut cx, 0, "text")?;
      let x = float_arg(&mut cx, 1, "x")?;
      let y = float_arg(&mut cx, 2, "y")?;
      let width = opt_float_arg(&mut cx, 3);

      if width.is_none() && cx.len() > 3 && !cx.argument::<JsValue>(3)?.is_a::<JsUndefined>(){
        // it's fine to include an ignored `undefined` but anything else is invalid
        return Ok(cx.undefined().upcast())
      }

      cx.borrow_mut(&mut this, |mut this|{
        let paint = this.paint_for_stroke();
        this.draw_text(&text, x, y, width, paint);
      });

      Ok(cx.undefined().upcast())
    }

    method _measureText(mut cx){
      let mut this = cx.this();
      let text = string_arg(&mut cx, 0, "text")?;
      let width = opt_float_arg(&mut cx, 1);
      let text_metrics = cx.borrow_mut(&mut this, |mut this| this.measure_text(&text, width) );

      let results = JsArray::new(&mut cx, text_metrics.len() as u32);
      for (i, info) in text_metrics.iter().enumerate(){
        let line = floats_to_array(&mut cx, &info)?;
        results.set(&mut cx, i as u32, line)?;
      }
      Ok(results.upcast())
    }

    // -- type properties ---------------------------------------------------------------

    method get_font(mut cx){
      let this = cx.this();
      let font_str = cx.borrow(&this, |this| this.state.font.clone() );
      Ok(cx.string(font_str).upcast())
    }

    method set_font(mut cx){
      let mut this = cx.this();
      if let Some(spec) = font_arg(&mut cx, 0)?{
        cx.borrow_mut(&mut this, |mut this|{ this.set_font(spec) });
      }
      Ok(cx.undefined().upcast())
    }

    method get_textAlign(mut cx){
      let this = cx.this();
      let mode = cx.borrow(&this, |this| this.state.graf_style.text_align() );
      let name = from_text_align(mode);
      Ok(cx.string(name).upcast())

    }

    method set_textAlign(mut cx){
      let mut this = cx.this();
      let name = string_arg(&mut cx, 0, "textAlign")?;
      if let Some(mode) = to_text_align(&name){
        cx.borrow_mut(&mut this, |mut this|{
          this.state.graf_style.set_text_align(mode);
        });
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

    method get_direction(mut cx){
      let this = cx.this();
      let name = cx.borrow(&this, |this|
        match this.state.graf_style.text_direction(){
          TextDirection::LTR => "ltr",
          TextDirection::RTL => "rtl",
        }
      );
      Ok(cx.string(name).upcast())
    }

    method set_direction(mut cx){
      let mut this = cx.this();
      let name = string_arg(&mut cx, 0, "direction")?;

      let direction = match name.to_lowercase().as_str(){
        "ltr" => Some(TextDirection::LTR),
        "rtl" => Some(TextDirection::RTL),
        _ => None
      };

      if let Some(dir) = direction{
        cx.borrow_mut(&mut this, |mut this|{
          this.state.graf_style.set_text_direction(dir);
        })
      }

      Ok(cx.undefined().upcast())
    }

    // -- non-standard typography extensions --------------------------------------------

    method get_fontVariant(mut cx){
      let this = cx.this();
      let font_str = cx.borrow(&this, |this| this.state.font_variant.clone() );
      Ok(cx.string(font_str).upcast())
    }

    method set_fontVariant(mut cx){
      let mut this = cx.this();
      let arg = cx.argument::<JsObject>(0)?;
      let variant = string_for_key(&mut cx, &arg, "variant")?;
      let feat_obj = arg.get(&mut cx, "features")?.downcast_or_throw::<JsObject, _>(&mut cx)?;
      let features = font_features(&mut cx, &feat_obj)?;
      cx.borrow_mut(&mut this, |mut this|{
        this.set_font_variant(&variant, &features);
      });
      Ok(cx.undefined().upcast())
    }

    method get_textTracking(mut cx){
      let this = cx.this();
      let tracking = cx.borrow(&this, |this| this.state.text_tracking );
      Ok(cx.number(tracking).upcast())
    }

    method set_textTracking(mut cx){
      let mut this = cx.this();
      let tracking = float_arg(&mut cx, 0, "tracking")?;
      cx.borrow_mut(&mut this, |mut this|{
        let em = this.state.char_style.font_size();
        this.state.text_tracking = tracking as i32;
        this.state.char_style.set_letter_spacing(tracking as f32 / 1000.0 * em);
      });
      Ok(cx.undefined().upcast())
    }

    method get_textWrap(mut cx){
      let this = cx.this();
      let flag = cx.borrow(&this, |this| this.state.text_wrap );
      Ok(cx.boolean(flag).upcast())
    }

    method set_textWrap(mut cx){
      let mut this = cx.this();
      let flag = bool_arg(&mut cx, 0, "textWrap")?;
      cx.borrow_mut(&mut this, |mut this| this.state.text_wrap = flag );
      Ok(cx.undefined().upcast())
    }


    //
    // Effects
    //

    // -- css3 filters ------------------------------------------------------------------

    method get_filter(mut cx){
      let this = cx.this();
      let filter = cx.borrow(&this, |this| this.state.filter.clone() );
      Ok(cx.string(filter).upcast())
    }

    method set_filter(mut cx){
      let mut this = cx.this();
      if !cx.argument::<JsValue>(0)?.is_a::<JsNull>() {
        let (filter_text, filters) = filter_arg(&mut cx, 0)?;
        cx.borrow_mut(&mut this, |mut this|{
          this.set_filter(&filter_text, &filters);
        });
      }


      Ok(cx.undefined().upcast())
    }

    // -- compositing properties --------------------------------------------------------

    method get_globalAlpha(mut cx){
      let this = cx.this();
      let num = cx.borrow(&this, |this| this.state.global_alpha );
      Ok(cx.number(num as f64).upcast())
    }

    method set_globalAlpha(mut cx){
      let mut this = cx.this();
      let num = float_arg(&mut cx, 0, "globalAlpha")?;
      if (0.0..=1.0).contains(&num){
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
        cx.borrow_mut(&mut this, |mut this| {
          this.state.global_composite_operation = mode;
          this.state.paint.set_blend_mode(mode);
        });
      }
      Ok(cx.undefined().upcast())
    }

    // -- dropshadow properties ---------------------------------------------------------

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
      color_to_css(&mut cx, &shadow_color)
    }

    method set_shadowColor(mut cx){
      let mut this = cx.this();
      if let Some(color) = color_arg(&mut cx, 0){
        cx.borrow_mut(&mut this, |mut this| { this.state.shadow_color = color; });
      }else{
        eprintln!("Warning: Invalid shadow color (expected a css color string)");
      }

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

 }
}