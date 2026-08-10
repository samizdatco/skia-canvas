use neon::prelude::*;
use color::{parse_color, ColorSpaceTag, DynamicColor, Srgb};
use skia_safe::{Color, Color4f, ColorSpace, RGB};

//
// Colors
//

#[derive(Clone, Copy, Debug)]
pub struct CssColor{
  pub color: Color4f, // the wide-gamut (extended-sRGB) color
  parsed: DynamicColor, // the canonicalized string form (for returning via getters)
}

impl CssColor{
  pub fn black() -> Self{ CssColor::parse("black").unwrap() }
  pub fn transparent() -> Self{ CssColor::parse("transparent").unwrap() }

  pub fn parse(css:&str) -> Option<Self>{
    // linebender covers the full CSS Color 4 syntax (oklch, lab, color(display-p3 ...), etc)
    parse_color(css).ok().map(|mut parsed|{
      // clamp % components before conversion (as the spec requires) since linebender leaves them unbounded
      if matches!(parsed.cs, ColorSpaceTag::Hsl | ColorSpaceTag::Hwb){
        parsed.components[1] = parsed.components[1].clamp(0.0, 100.0);
        parsed.components[2] = parsed.components[2].clamp(0.0, 100.0);
      }
      // convert to extended sRGB (out-of-gamut components are unclamped until rasterization)
      let [red, green, blue, alpha] = parsed.to_alpha_color::<Srgb>().components;
      Self{ color:Color4f::new(red, green, blue, alpha.clamp(0.0, 1.0)), parsed }
    })
  }

  pub fn to_css(&self) -> String{
    // return canonical string (use legacy format for hex/rgba(), CSS Color 4 syntax for everything else)
    if self.parsed.flags.named()
      && matches!(self.parsed.cs, ColorSpaceTag::Srgb | ColorSpaceTag::Hsl | ColorSpaceTag::Hwb)
    {
      let color = color4f_to_color(self.color);
      let RGB {r, g, b} = color.to_rgb();
      if self.color.a >= 1.0{
        format!("#{:02x}{:02x}{:02x}", r, g, b)
      }else{
        let alpha = format!("{:.3}", self.color.a);
        let alpha = alpha.trim_end_matches('0').trim_end_matches('.');
        format!("rgba({}, {}, {}, {})", r, g, b, alpha)
      }
    }else{
      self.parsed.to_string()
    }
  }
}

pub fn css_to_color4f(css:&str) -> Option<Color4f> {
  CssColor::parse(css).map(|css_color| css_color.color)
}

pub fn color4f_to_color(color:Color4f) -> Color {
  // clamp each component to 8-bit sRGB (emulating browser behavior)
  Color::from_argb(
    (color.a.clamp(0.0, 1.0)*255.0).round() as u8,
    (color.r.clamp(0.0, 1.0)*255.0).round() as u8,
    (color.g.clamp(0.0, 1.0)*255.0).round() as u8,
    (color.b.clamp(0.0, 1.0)*255.0).round() as u8,
  )
}

fn css_in<'a>(cx: &mut FunctionContext<'a>, val: Handle<'a, JsValue>) -> Option<String> {
  if val.is_a::<JsString, _>(cx) {
    Some(val.downcast::<JsString, _>(cx).unwrap().value(cx))
  }else{
    // for other objects, try calling their .toString() method (if it exists)
    let obj = val.downcast::<JsObject, _>(cx).ok()?;
    let attr = obj.get::<JsValue, _, _>(cx, "toString").ok()?;
    let to_string = attr.downcast::<JsFunction, _>(cx).ok()?;
    let result = to_string.call(cx, obj, vec![]).ok()?;
    result.downcast::<JsString, _>(cx).ok().map(|css| css.value(cx))
  }
}

pub fn color_in_4f<'a>(cx: &mut FunctionContext<'a>, val: Handle<'a, JsValue>) -> Option<Color4f> {
  css_in(cx, val).and_then(|css| css_to_color4f(&css))
}

pub fn css_color_in<'a>(cx: &mut FunctionContext<'a>, val: Handle<'a, JsValue>) -> Option<CssColor> {
  css_in(cx, val).and_then(|css| CssColor::parse(&css))
}

pub fn opt_color_arg_4f(cx: &mut FunctionContext, idx: usize) -> Option<Color4f> {
  match cx.argument_opt(idx) {
    Some(arg) => color_in_4f(cx, arg),
    _ => None
  }
}

pub fn opt_css_color_arg(cx: &mut FunctionContext, idx: usize) -> Option<CssColor> {
  match cx.argument_opt(idx) {
    Some(arg) => css_color_in(cx, arg),
    _ => None
  }
}

pub fn opt_color_4f_for_key(cx: &mut FunctionContext, obj: &Handle<JsObject>, attr:&str) -> Option<Color4f>{
  obj.get(cx, attr).ok()
    .and_then(|val|
      color_in_4f(cx, val)
    )
}

use once_cell::sync::Lazy;
use skia_safe::{named_primaries, named_transfer_fn};

static SRGB_COLOR_SPACE: Lazy<ColorSpace> = Lazy::new(ColorSpace::new_srgb);
static DISPLAY_P3_COLOR_SPACE: Lazy<ColorSpace> = Lazy::new(||
  // Display P3 = P3-D65 primaries + the sRGB transfer curve
  ColorSpace::new_cicp(named_primaries::CicpId::SMPTE_EG_432_1, named_transfer_fn::CicpId::SRGB)
    .expect("Could not construct display-p3 color space")
);

pub fn to_color_space(mode_name:&str) -> ColorSpace{
  match mode_name{
    "display-p3" => DISPLAY_P3_COLOR_SPACE.clone(),
    "srgb" | _ => SRGB_COLOR_SPACE.clone()
  }
}

#[allow(dead_code)] // kept as the counterpart to to_color_space
pub fn from_color_space(mode:ColorSpace) -> String{
  match mode {
    p3 if p3 == *DISPLAY_P3_COLOR_SPACE => "display-p3",
    _ => "srgb"
  }.to_string()
}

use skia_safe::gradient::{Interpolation, interpolation::{ColorSpace as InterpColorSpace, HueMethod, InPremul}};

// The spec-default gradient interpolation: non-premultiplied, sRGB, shorter-hue
pub fn default_interpolation() -> Interpolation{
  Interpolation{ in_premul:InPremul::No, color_space:InterpColorSpace::SRGB, hue_method:HueMethod::Shorter }
}

pub fn color_interpolation_from_str(name:&str) -> Option<InterpColorSpace>{
  Some(match name.trim().to_lowercase().as_str(){
    "srgb"         => InterpColorSpace::SRGB,
    "srgb-linear"  => InterpColorSpace::SRGBLinear,
    "display-p3"   => InterpColorSpace::DisplayP3,
    "a98-rgb"      => InterpColorSpace::A98RGB,
    "prophoto-rgb" => InterpColorSpace::ProphotoRGB,
    "rec2020"      => InterpColorSpace::Rec2020,
    "lab"          => InterpColorSpace::Lab,
    "oklab"        => InterpColorSpace::OKLab,
    "lch"          => InterpColorSpace::LCH,
    "oklch"        => InterpColorSpace::OKLCH,
    "hsl"          => InterpColorSpace::HSL,
    "hwb"          => InterpColorSpace::HWB,
    _ => return None,
  })
}

pub fn color_interpolation_to_str(space:InterpColorSpace) -> &'static str{
  match space{
    InterpColorSpace::SRGBLinear    => "srgb-linear",
    InterpColorSpace::DisplayP3     => "display-p3",
    InterpColorSpace::A98RGB        => "a98-rgb",
    InterpColorSpace::ProphotoRGB   => "prophoto-rgb",
    InterpColorSpace::Rec2020       => "rec2020",
    InterpColorSpace::Lab           => "lab",
    InterpColorSpace::OKLab | InterpColorSpace::OKLabGamutMap => "oklab",
    InterpColorSpace::LCH           => "lch",
    InterpColorSpace::OKLCH | InterpColorSpace::OKLCHGamutMap => "oklch",
    InterpColorSpace::HSL           => "hsl",
    InterpColorSpace::HWB           => "hwb",
    _ => "srgb",
  }
}

pub fn hue_method_from_str(name:&str) -> Option<HueMethod>{
  Some(match name.trim().to_lowercase().as_str(){
    "shorter"    => HueMethod::Shorter,
    "longer"     => HueMethod::Longer,
    "increasing" => HueMethod::Increasing,
    "decreasing" => HueMethod::Decreasing,
    _ => return None,
  })
}

pub fn hue_method_to_str(method:HueMethod) -> &'static str{
  match method{
    HueMethod::Longer     => "longer",
    HueMethod::Increasing => "increasing",
    HueMethod::Decreasing => "decreasing",
    _ => "shorter",
  }
}
