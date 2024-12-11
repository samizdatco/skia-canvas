#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]
use neon::prelude::*;

use crate::utils::*;
use crate::context::BoxedContext2D;
use crate::gpu::RenderingEngine;

pub mod app;
use app::{App, LoopMode};

pub mod window;
use window::WindowSpec;

pub mod event;

pub mod window_mgr;

fn validate_gpu(cx:&mut FunctionContext) -> NeonResult<()>{
    // bail out if we can't draw to the screen
    if let Some(reason) = RenderingEngine::default().lacks_gpu_support(){
        cx.throw_error(reason)?
    }
    Ok(())
}

pub fn register(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    let callback = cx.argument::<JsFunction>(1)?.root(&mut cx);
    App::register(callback);
    Ok(cx.undefined())
}


pub fn activate(mut cx: FunctionContext) -> JsResult<JsPromise> {
    validate_gpu(&mut cx)?;

    let (deferred, promise) = cx.promise();
    let channel = cx.channel();

    App::activate(channel, deferred);

    Ok(promise)
}

pub fn set_rate(mut cx: FunctionContext) -> JsResult<JsNumber> {
    let fps = float_arg(&mut cx, 1, "framesPerSecond")?;
    App::set_fps(fps);
    Ok(cx.number(fps as f64))
}

pub fn set_mode(mut cx: FunctionContext) -> JsResult<JsString> {
    let mode = string_arg(&mut cx, 1, "eventLoopMode")?;
    let loop_mode = match mode.as_str(){
        "node" => Ok(LoopMode::Node),
        "native" => Ok(LoopMode::Native),
        _ => cx.throw_error(format!("Invalid event loop mode: {}", mode))
    }?;

    App::set_mode(loop_mode);
    Ok(cx.string(mode))
}

pub fn open(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    let win_config = string_arg(&mut cx, 1, "Window configuration")?;
    let context = cx.argument::<BoxedContext2D>(2)?;
    let spec = serde_json::from_str::<WindowSpec>(&win_config).expect("Invalid window state");

    validate_gpu(&mut cx)?;

    App::open_window(spec, context.borrow().get_page());
    Ok(cx.undefined())
}

pub fn close(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    let token = float_arg(&mut cx, 1, "windowID")? as u32;
    App::close_window(token);
    Ok(cx.undefined())
}

pub fn quit(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    App::quit();
    Ok(cx.undefined())
}
