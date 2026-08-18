use neon::prelude::*;
use skia_safe::{Color4f, Data};

use super::*;

//
// Image & ImageData
//

use crate::image::ImageData;
use neon::types::buffer::TypedArray;
use skia_safe::{ColorType, ColorSpace, ImageInfo, AlphaType};

// truncate toward zero rather than flooring (following the spec for get/setImageData coords)
pub fn long_args_at(cx: &mut FunctionContext, start:usize, names:&[&str]) -> NeonResult<Vec<f32>>{
  let nums = float_args_at(cx, start, names)?;
  for (i, (num, name)) in nums.iter().zip(names).enumerate(){
    if *num < i32::MIN as f32 || *num > i32::MAX as f32 {
      return cx.throw_type_error(
        format!("Expected an integer for `{}` as {} arg", name, arg_num(start + i))
      );
    }
  }
  Ok(nums.iter().map(|num| num.trunc()).collect())
}

pub fn opt_image_info_arg(cx: &mut FunctionContext, idx:usize) -> NeonResult<Option<ImageInfo>>{
  if let Some(raw_info) = opt_object_arg(cx, idx){
     Ok(Some(ImageInfo::new(
        (
          float_for_key(cx, &raw_info, "width")? as _,
          float_for_key(cx, &raw_info, "height")? as _
        ),
        ColorType::RGBA8888,
        match bool_for_key(cx, &raw_info, "premultiplied")?{
          false => AlphaType::Unpremul,
          true => AlphaType::Premul
        },
        ColorSpace::new_srgb(),
      )))
  }else{
    Ok(None)
  }
}

pub fn image_data_arg(cx: &mut FunctionContext, idx:usize) -> NeonResult<ImageData>{
  let obj = object_arg(cx, idx, "imageData")?;
  let width = float_for_key(cx, &obj, "width")?;
  let height = float_for_key(cx, &obj, "height")?;
  let color_type = string_for_key(cx, &obj, "colorType")?;
  let color_space = string_for_key(cx, &obj, "colorSpace")?;
  let js_buffer: Handle<JsBuffer> = obj.get(cx, "data")?;
  let buffer = Data::new_copy(js_buffer.as_slice(cx));

  Ok(ImageData::new(buffer, width, height, color_type, color_space))
}

pub fn image_data_settings_arg(cx: &mut FunctionContext, idx:usize) -> (ColorType, ColorSpace){
  match opt_object_arg(cx, idx){
    Some(obj) => {
      let color_type = opt_string_for_key(cx, &obj, "colorType").unwrap_or("rgba".to_string());
      let color_space = opt_string_for_key(cx, &obj, "colorSpace").unwrap_or("srgb".to_string());
      (to_color_type(&color_type), to_color_space(&color_space))
    }
    None => (ColorType::RGBA8888, ColorSpace::new_srgb())
  }
}

pub fn image_data_export_arg(cx: &mut FunctionContext, idx:usize) -> (ColorType, Option<ColorSpace>, Option<Color4f>, f32, Option<usize>){
  match opt_object_arg(cx, idx){
    Some(obj) => {
      let color_type = opt_string_for_key(cx, &obj, "colorType").unwrap_or("rgba".to_string());
      let color_space = opt_string_for_key(cx, &obj, "colorSpace").map(|name| to_color_space(&name));
      let matte = opt_color_4f_for_key(cx, &obj, "matte");
      let density = opt_float_for_key(cx, &obj, "density").unwrap_or(1.0);
      let msaa = opt_float_for_key(cx, &obj, "msaa").map(|n| n as usize);
      (to_color_type(&color_type), color_space, matte, density, msaa)
    }
    None => (ColorType::RGBA8888, None, None, 1.0, None)
  }
}

pub fn to_color_type(type_name: &str) -> ColorType {
  match type_name {
    "Alpha8" => ColorType::Alpha8,
    "RGB565" => ColorType::RGB565,
    "ARGB4444" => ColorType::ARGB4444,
    "RGBA1010102" => ColorType::RGBA1010102,
    "BGRA1010102" => ColorType::BGRA1010102,
    "RGB101010x" => ColorType::RGB101010x,
    "BGR101010x" => ColorType::BGR101010x,
    "Gray8" => ColorType::Gray8,
    "RGBAF16Norm" => ColorType::RGBAF16Norm,
    "RGBAF16" => ColorType::RGBAF16,
    "RGBAF32" => ColorType::RGBAF32,
    "R8G8UNorm" => ColorType::R8G8UNorm,
    "A16Float" => ColorType::A16Float,
    "R16G16Float" => ColorType::R16G16Float,
    "A16UNorm" => ColorType::A16UNorm,
    "R16G16UNorm" => ColorType::R16G16UNorm,
    "R16G16B16A16UNorm" => ColorType::R16G16B16A16UNorm,
    "SRGBA8888" => ColorType::SRGBA8888,
    "R8UNorm" => ColorType::R8UNorm,
    "N32" => ColorType::N32,
    "RGB888x"|"rgb" => ColorType::RGB888x,
    "BGRA8888"|"bgra" => ColorType::BGRA8888,
    "RGBA8888"|"rgba"|_ => ColorType::RGBA8888,
  }
}

// pub fn from_color_type(color_type: ColorType) -> String {
//   match color_type {
//     ColorType::Alpha8 => "Alpha8",
//     ColorType::RGB565 => "RGB565",
//     ColorType::ARGB4444 => "ARGB4444",
//     ColorType::RGBA8888 => "RGBA8888",
//     ColorType::RGB888x => "RGB888x",
//     ColorType::BGRA8888 => "BGRA8888",
//     ColorType::RGBA1010102 => "RGBA1010102",
//     ColorType::BGRA1010102 => "BGRA1010102",
//     ColorType::RGB101010x => "RGB101010x",
//     ColorType::BGR101010x => "BGR101010x",
//     ColorType::Gray8 => "Gray8",
//     ColorType::RGBAF16Norm => "RGBAF16Norm",
//     ColorType::RGBAF16 => "RGBAF16",
//     ColorType::RGBAF32 => "RGBAF32",
//     ColorType::R8G8UNorm => "R8G8UNorm",
//     ColorType::A16Float => "A16Float",
//     ColorType::R16G16Float => "R16G16Float",
//     ColorType::A16UNorm => "A16UNorm",
//     ColorType::R16G16UNorm => "R16G16UNorm",
//     ColorType::R16G16B16A16UNorm => "R16G16B16A16UNorm",
//     ColorType::SRGBA8888 => "SRGBA8888",
//     ColorType::R8UNorm => "R8UNorm",
//     _ => "unknown"
//   }.to_string()
// }

//
// ExportOptions
//

use crate::gfx::page::ExportOptions;

pub fn export_options_arg(cx: &mut FunctionContext, idx: usize) -> NeonResult<ExportOptions>{
  let opts = opt_object_arg(cx, idx).unwrap();
  let format = string_for_key(cx, &opts, "format")?;
  let quality = float_for_key(cx, &opts, "quality")?;
  let density = float_for_key(cx, &opts, "density")?;
  let jpeg_downsample = bool_for_key(cx, &opts, "downsample")?;
  let matte = opt_color_4f_for_key(cx, &opts, "matte");
  let msaa = opt_float_for_key(cx, &opts, "msaa")
    .map(|num| num.floor() as usize);
  let color_type = opt_string_for_key(cx, &opts, "colorType")
    .map(|mode| to_color_type(&mode)).unwrap_or(ColorType::RGBA8888);
  let text_contrast = float_for_key(cx, &opts, "textContrast")?;
  let text_gamma = float_for_key(cx, &opts, "textGamma")?;
  let outline = bool_for_key(cx, &opts, "outline")?;

  Ok(ExportOptions{
    format, quality, density, outline, matte, msaa, color_type, jpeg_downsample, text_contrast, text_gamma
  })
}
