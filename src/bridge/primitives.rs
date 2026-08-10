use std::cmp;
use core::ops::Range;
use neon::prelude::*;

use super::arg_num;

//
// plain objects
//

pub fn opt_object_arg<'a>(cx: &mut FunctionContext<'a>, idx:usize) -> Option<Handle<'a, JsObject>>{
  match cx.argument_opt(idx) {
    Some(arg) => match arg.downcast::<JsObject, _>(cx) {
      Ok(obj) => Some(obj),
      Err(_e) => None
    },
    None => None
  }
}

pub fn object_arg<'a>(cx: &mut FunctionContext<'a>, idx:usize, attr:&str) -> NeonResult<Handle<'a, JsObject>>{
  match opt_object_arg(cx, idx){
    Some(val) => Ok(val),
    None => cx.throw_type_error(format!("Exptected an object for \"{}\"", attr))
  }
}

pub fn opt_object_for_key<'a>(cx: &mut FunctionContext<'a>, obj: &Handle<'a, JsObject>, attr:&str) -> Option<Handle<'a, JsObject>>{
  if let Some(val) = obj.get::<JsValue, _, _>(cx, attr).ok(){
    return val.downcast::<JsObject, _>(cx).ok()
  }
  None
}

#[allow(dead_code)] // kept as the throwing counterpart to opt_object_for_key
pub fn object_for_key<'a>(cx: &mut FunctionContext<'a>, obj: &Handle<'a, JsObject>, attr:&str) -> NeonResult<Handle<'a, JsObject>>{
  match opt_object_for_key(cx, &obj, attr){
    Some(val) => Ok(val),
    None => cx.throw_type_error(format!("Exptected an object for \"{}\"", attr))
  }
}

//
// strings
//

pub fn strings_in(cx: &mut FunctionContext, vals: &[Handle<JsValue>]) -> Vec<String>{
  let mut strs:Vec<String> = Vec::new();
  for val in vals.iter() {
    if let Ok(txt) = val.downcast::<JsString, _>(cx){
      let val = txt.value(cx);
      strs.push(val);
    }
  }
  strs
}

pub fn strings_at_key(cx: &mut FunctionContext, obj: &Handle<JsObject>, attr:&str) -> NeonResult<Vec<String>>{
  let array:Handle<JsArray> = obj.get(cx, attr)?;
  let list = array.to_vec(cx)?;
  Ok(strings_in(cx, &list))
}

pub fn opt_string_for_key(cx: &mut FunctionContext, obj: &Handle<JsObject>, attr:&str) -> Option<String>{
  obj.get(cx, attr).ok()
    .and_then(|val:Handle<JsValue>| val.downcast::<JsString, _>(cx).ok() )
    .map(|v| v.value(cx))
}

pub fn string_for_key(cx: &mut FunctionContext, obj: &Handle<JsObject>, attr:&str) -> NeonResult<String>{
  let key = cx.string(attr);
  let val:Handle<JsValue> = obj.get(cx, key)?;
  match val.downcast::<JsString, _>(cx){
    Ok(s) => Ok(s.value(cx)),
    Err(_e) => cx.throw_type_error(format!("Exptected a string for \"{}\"", attr))
  }
}

pub fn opt_string_arg(cx: &mut FunctionContext, idx: usize) -> Option<String>{
  match cx.argument_opt(idx) {
    Some(arg) => match arg.downcast::<JsString, _>(cx) {
      Ok(v) => Some(v.value(cx)),
      Err(_e) => None
    },
    None => None
  }
}

pub fn string_arg(cx: &mut FunctionContext, idx: usize, attr:&str) -> NeonResult<String> {
  let exists = cx.len() > idx;
  match opt_string_arg(cx, idx){
    Some(v) => Ok(v),
    None => cx.throw_type_error(
      if exists { format!("Expected a string for `{}`", attr) }
      else { format!("not enough arguments: expected a string for `{}` as {} arg", attr, arg_num(idx)) }
    )
  }
}

pub fn strings_to_array<'a>(cx: &mut FunctionContext<'a>, strings: &[String]) -> JsResult<'a, JsArray> {
  let array = JsArray::new(cx, strings.len());
  for (i, val) in strings.iter().enumerate() {
    let num = cx.string(val.as_str());
    array.set(cx, i as u32, num)?;
  }
  Ok(array)
}

//
// bools
//

pub fn opt_bool_arg(cx: &mut FunctionContext, idx: usize) -> Option<bool>{
  match cx.argument_opt(idx) {
    Some(arg) => match arg.downcast::<JsBoolean, _>(cx) {
      Ok(v) => Some(v.value(cx)),
      Err(_e) => None
    },
    None => None
  }
}

pub fn bool_arg_or(cx: &mut FunctionContext, idx: usize, default:bool) -> bool{
  opt_bool_arg(cx, idx).unwrap_or(default)
}

pub fn bool_arg(cx: &mut FunctionContext, idx: usize, attr:&str) -> NeonResult<bool>{
  let exists = cx.len() > idx;
  match opt_bool_arg(cx, idx){
    Some(v) => Ok(v),
    None => cx.throw_type_error(
      if exists { format!("{} must be a boolean", attr) }
      else { format!("not enough arguments: expected a boolean for `{}` as {} arg", attr, arg_num(idx)) }
    )
  }
}

pub fn bool_for_key(cx: &mut FunctionContext, obj: &Handle<JsObject>, attr:&str) -> NeonResult<bool>{
  let key = cx.string(attr);
  let val:Handle<JsValue> = obj.get(cx, key)?;
  match val.downcast::<JsBoolean, _>(cx){
    Ok(v) => Ok(v.value(cx) as bool),
    Err(_e) => cx.throw_type_error(format!("Exptected a boolean value for \"{}\"", attr))
  }
}

//
// floats
//


fn _as_double(cx: &mut FunctionContext, val:&Handle<JsValue>) -> Option<f64>{
  // emulate (some of) javascript's wildly permissive type coercion <https://www.w3schools.com/js/js_type_conversion.asp>
  val.downcast::<JsNumber, _>(cx).ok().map(|num|{
    num.value(cx) as f64
  }).or_else(||{
    // strings
    val.downcast::<JsString, _>(cx).ok().and_then(|txt|{
      let s = txt.value(cx);
      if let Some(s) = s.strip_prefix("0x"){
        u64::from_str_radix(s, 16).map(|i| i as f64).ok()
      }else if let Some(s) = s.strip_prefix("0o"){
        u64::from_str_radix(s, 8).map(|i| i as f64).ok()
      }else if let Some(s) = s.strip_prefix("0b"){
        u64::from_str_radix(s, 2).map(|i| i as f64).ok()
      }else if s.is_empty(){
        Some(0.0)
      }else{
        s.parse::<f64>().ok()
      }
    })
  }).or_else(||{
    // booleans
    val.downcast::<JsBoolean, _>(cx).ok().map(|b| match b.value(cx) {
      true => 1.0,
      false => 0.0
    })
  }).or_else(||{
    // null
    val.downcast::<JsNull, _>(cx).ok().map(|_| 0.0)
  }).or_else(||{
    // arrays
    val.downcast::<JsArray, _>(cx).ok().and_then(|array|
      match array.len(cx) {
        0 => Some(0.0),
        1 => array.to_vec(cx).ok().and_then(|nums| _as_double(cx, &nums[0])),
        _ => None
      })
  }).and_then(|num| match num.is_finite(){
    true => Some(num),
    false => None
  })
}

fn _as_float(cx: &mut FunctionContext, val:&Handle<JsValue>) -> Option<f32>{
  _as_double(cx, val).map(|num| num as f32)
}

pub fn _float_args_at(cx: &mut FunctionContext, start:usize, names:&[&str], or_bail:bool) -> NeonResult<Vec<f32>>{
  let argc = cx.len() - start; // args start after the `this` reference
  if argc < names.len() {
    return cx.throw_type_error(format!("not enough arguments (missing: {})", names[argc..].join(", ")));
  }

  // emoji indicates that it will only throw in strict mode
  let prefix = if or_bail{ "⚠️" }else{ "" };

  let mut args:Vec<f32> = Vec::new();
  for (i, name) in names.iter().enumerate(){
    match opt_float_arg(cx, i+start){
      Some(v) => args.push(v),
      None => return cx.throw_type_error(
        format!("{}Expected a number for `{}` as {} arg", prefix, name, arg_num(i+start))
      )
    }
  }

  Ok(args)
}

pub fn opt_double_for_key(cx: &mut FunctionContext, obj: &Handle<JsObject>, attr:&str) -> Option<f64>{
  obj.get(cx, attr).ok().and_then(|val| _as_double(cx, &val))
}

pub fn opt_float_for_key(cx: &mut FunctionContext, obj: &Handle<JsObject>, attr:&str) -> Option<f32>{
  obj.get(cx, attr).ok().and_then(|val| _as_float(cx, &val))
}

pub fn float_for_key(cx: &mut FunctionContext, obj: &Handle<JsObject>, attr:&str) -> NeonResult<f32>{
  match opt_float_for_key(cx, &obj, attr) {
    Some(num) => Ok(num),
    None => cx.throw_type_error(format!("Exptected a numerical value for \"{}\"", attr))
  }
}

pub fn floats_in(cx: &mut FunctionContext, vals: &[Handle<JsValue>]) -> Vec<f32>{
  vals.iter().filter_map(|val| _as_float(cx, val)).collect::<Vec<f32>>()
}

pub fn opt_float_arg(cx: &mut FunctionContext, idx: usize) -> Option<f32>{
  cx.argument_opt(idx).and_then(|val| _as_float(cx, &val))
}

pub fn float_arg_or(cx: &mut FunctionContext, idx: usize, default:f32) -> f32{
  opt_float_arg(cx, idx).unwrap_or(default)
}

pub fn float_arg(cx: &mut FunctionContext, idx: usize, attr:&str) -> NeonResult<f32>{
  _float_args_at(cx, idx, &[attr], false)
    .map(|vec| vec.into_iter().nth(0).unwrap())
}


pub fn float_arg_or_bail(cx: &mut FunctionContext, idx: usize, attr:&str) -> NeonResult<f32>{
  _float_args_at(cx, idx, &[attr], true)
    .map(|vec| vec.into_iter().nth(0).unwrap())
}

pub fn floats_to_array<'a>(cx: &mut FunctionContext<'a>, nums: &[f32]) -> JsResult<'a, JsValue> {
  let array = JsArray::new(cx, nums.len());
  for (i, val) in nums.iter().enumerate() {
    let num = cx.number(*val);
    array.set(cx, i as u32, num)?;
  }
  Ok(array.upcast())
}

//
// float spreads
//

pub fn opt_float_args(cx: &mut FunctionContext, rng: Range<usize>) -> Vec<f32>{
  let end = cmp::min(rng.end, cx.len() as usize);
  let rng = rng.start..end;

  let mut args:Vec<f32> = Vec::new();
  for i in rng.start..end{
    if let Some(val) = opt_float_arg(cx, i){
      args.push(val);
    }
  }
  args
}

pub fn float_args(cx: &mut FunctionContext, names:&[&str]) -> NeonResult<Vec<f32>>{
  _float_args_at(cx, 1, names, false)
}

pub fn float_args_at(cx: &mut FunctionContext, start:usize, names:&[&str]) -> NeonResult<Vec<f32>>{
  _float_args_at(cx, start, names, false)
}

pub fn float_args_or_bail_at(cx: &mut FunctionContext, start:usize, names:&[&str]) -> NeonResult<Vec<f32>>{
  _float_args_at(cx, start, names, true)
}
