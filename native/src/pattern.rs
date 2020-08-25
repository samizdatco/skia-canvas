#![allow(unused_imports)]
use std::rc::Rc;
use std::cell::RefCell;
use neon::prelude::*;
use skia_safe::{shaders, Shader, Matrix, TileMode::{Decal, Repeat}};

use crate::utils::*;
use crate::image::{Image, JsImage};
use crate::canvas::{Canvas, JsCanvas, canvas_pages};

#[derive(Clone)]
pub struct CanvasPattern{
  stamp: Shader,
  shader: Rc<RefCell<Shader>>
}

impl CanvasPattern{
  pub fn shader(&self) -> Option<Shader>{
    Some(self.shader.borrow().clone())
  }
}

declare_types! {
  pub class JsCanvasPattern for CanvasPattern {
    init(mut cx) {
      let src = cx.argument::<JsValue>(0)?;
      let repetition = if cx.len() > 1 && cx.argument::<JsValue>(1)?.is_a::<JsNull>(){
        "".to_string() // null is a valid synonym for "repeat" (as is "")
      }else{
        string_arg(&mut cx, 1, "repetition")?
      };

      let (tile_x, tile_y) = match repetition.as_str() {
        "repeat" | "" => (Repeat, Repeat),
        "repeat-x" => (Repeat, Decal),
        "repeat-y" => (Decal, Repeat),
        "no-repeat" => (Decal, Decal),
        _ => return cx.throw_error("Unknown pattern repeat style")
      };

      let shader = match src {
        src if src.is_a::<JsImage>() => {
          let src = cx.argument::<JsImage>(0)?;
          cx.borrow(&src, |src| {
            src.image.as_ref().map(|image| image.to_shader((tile_x, tile_y), None))
          })
        }
        src if src.is_a::<JsCanvas>() => {
          let src = cx.argument::<JsCanvas>(0)?;
          let mut context = canvas_pages(&mut cx, &src)?[0];
          cx.borrow_mut(&mut context, |mut ctx| {
            ctx.get_picture(None).map(|pict|
              pict.to_shader((tile_x, tile_y), None, None)
            )
          })
        }
        _ => None
      };

      match shader {
        Some(stamp) => Ok(CanvasPattern{
          shader: Rc::new(RefCell::new(stamp.clone())), stamp
        }),
        None => cx.throw_type_error("CanvasPatterns require a source Image or a Canvas")
      }
    }

    method _setTransform(mut cx){
      let this = cx.this();
      let matrix = matrix_arg(&mut cx, 0)?;
      cx.borrow(&this, |this| {
        this.shader.replace(this.stamp.with_local_matrix(&matrix));
      });
      Ok(cx.undefined().upcast())
    }

  }
}