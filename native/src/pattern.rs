use neon::prelude::*;
use skia_safe::{Shader, TileMode, TileMode::{Decal, Repeat}, SamplingOptions,
                Image as SkImage, Picture, Matrix, FilterQuality, FilterMode};

use crate::utils::*;
use crate::image::{JsImage};
use crate::canvas::{JsCanvas, canvas_pages};

#[derive(Clone)]
pub struct CanvasPattern{
  pub smoothing:bool,
  image:Option<SkImage>,
  pict:Option<Picture>,
  tile_mode:(TileMode, TileMode),
  matrix:Matrix
}

impl CanvasPattern{
  pub fn shader(&self) -> Option<Shader>{
    if let Some(image) = &self.image{
      let sampling = match self.smoothing{
        true => SamplingOptions::from_filter_quality(FilterQuality::High, None),
        false => SamplingOptions::default()
      };
      match image.to_shader(self.tile_mode, sampling, None){
        Some(shader) => Some(shader.with_local_matrix(&self.matrix)),
        None => None
      }
    }else if let Some(pict) = &self.pict{
      let shader = pict.to_shader(self.tile_mode, FilterMode::Linear, None, None);
      Some(shader.with_local_matrix(&self.matrix))
    }else{
      None
    }
  }
}

declare_types! {
  pub class JsCanvasPattern for CanvasPattern {
    init(mut cx) {
      let smoothing = true;
      let matrix = Matrix::new_identity();

      let src = cx.argument::<JsValue>(0)?;
      let repetition = if cx.len() > 1 && cx.argument::<JsValue>(1)?.is_a::<JsNull>(){
        "".to_string() // null is a valid synonym for "repeat" (as is "")
      }else{
        string_arg(&mut cx, 1, "repetition")?
      };

      let tile_mode = match repetition.as_str() {
        "repeat" | "" => (Repeat, Repeat),
        "repeat-x" => (Repeat, Decal),
        "repeat-y" => (Decal, Repeat),
        "no-repeat" => (Decal, Decal),
        _ => return cx.throw_error("Unknown pattern repeat style")
      };

      if src.is_a::<JsImage>() {
        let src = cx.argument::<JsImage>(0)?;
        Ok(CanvasPattern{
          image:cx.borrow(&src, |src| src.image.clone() ),
          pict:None,
          tile_mode,
          smoothing,
          matrix
        })
      }else if src.is_a::<JsCanvas>() {
        let src = cx.argument::<JsCanvas>(0)?;
        let mut context = canvas_pages(&mut cx, &src)?[0];
        Ok(CanvasPattern{
          image:None,
          pict:cx.borrow_mut(&mut context, |mut ctx| ctx.get_picture(None) ),
          tile_mode,
          smoothing,
          matrix
        })
      }else{
        cx.throw_type_error("CanvasPatterns require a source Image or a Canvas")
      }
    }

    method _setTransform(mut cx){
      let mut this = cx.this();
      let matrix = matrix_arg(&mut cx, 0)?;
      cx.borrow_mut(&mut this, |mut this| {
        this.matrix = matrix;
      });
      Ok(cx.undefined().upcast())
    }

  }
}