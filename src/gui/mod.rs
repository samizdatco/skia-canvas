#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]
use neon::prelude::*;
use std::{
    sync::{Arc, OnceLock},
    iter::zip,
    cell::RefCell,
    thread::sleep,
    time::Duration,
};
use serde_json::Value;
use winit::event_loop::{EventLoop, EventLoopProxy};

use crate::utils::*;
use crate::context::BoxedContext2D;

pub mod app;
use app::{App, LoopMode};

pub mod event;
use event::AppEvent;

pub mod window;
use window::WindowSpec;

pub mod window_mgr;
use window_mgr::WindowManager;

use crate::gpu::RenderingEngine;
use crate::context::page::Page;

thread_local!(
    static APP: RefCell<App> = RefCell::new(App::default());
    static EVENT_LOOP: RefCell<EventLoop<AppEvent>> = RefCell::new(EventLoop::with_user_event().build().unwrap());
    static PROXY: RefCell<EventLoopProxy<AppEvent>> = RefCell::new(EVENT_LOOP.with_borrow(|event_loop|
        event_loop.create_proxy()
    ));
);

static RENDER_CALLBACK: OnceLock<Arc<Root<JsFunction>>> = OnceLock::new();

pub(crate) fn add_event(event: AppEvent){
    PROXY.with_borrow_mut(|proxy| proxy.send_event(event).ok() );
}

fn validate_gpu(cx:&mut FunctionContext) -> NeonResult<()>{
    // bail out if we can't draw to the screen
    if let Some(reason) = RenderingEngine::default().lacks_gpu_support(){
        cx.throw_error(reason)?
    }
    Ok(())
}

pub fn register(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    let callback = cx.argument::<JsFunction>(1)?.root(&mut cx);
    RENDER_CALLBACK.get_or_init(|| Arc::new(callback));

    Ok(cx.undefined())
}

fn dispatch_events(cx:&mut TaskContext, payload:Value, is_render:bool) -> NeonResult<Vec<(WindowSpec, Page)>>{
    // send payload to js for event dispatch and canvas drawing
    let mut call = match RENDER_CALLBACK.get(){
        None => return Ok(vec![]),
        Some(callback)=> callback.to_inner(cx).call_with(cx),
    };
    call.arg(cx.boolean(is_render))
        .arg(cx.string(payload.to_string()));

    match is_render{
        true => {
            // for a full roundtrip, pass events to js then unpack updated window specs & contexts
            let response = call.apply::<JsValue, _>(cx)?
                .downcast::<JsArray, _>(cx).or_throw(cx)?
                .to_vec(cx)?;

            let specs_json = response[0].downcast::<JsString, _>(cx).or_throw(cx)?.value(cx);
            let specs:Vec<WindowSpec> = serde_json::from_str(&specs_json)
                .or_else(|err| cx.throw_error("Malformed response from window event handler") )?;

            let contexts = response[1].downcast::<JsArray, _>(cx).or_throw(cx)?.to_vec(cx)?;
            let pages = contexts.iter().map(|boxed|
                boxed.downcast::<BoxedContext2D, _>(cx).ok()
                    .map(|ctx| ctx.borrow().get_page())
            );

            // group spec + page pairs for each window
            let window_state = zip(specs, pages)
                .filter_map(|(spec, page)| page.map(|page| (spec, page) ))
                .collect();

            Ok(window_state)
        }

        false => {
            // if this is just a UI-event delivery pass, ignore the js callback's return value
            call.exec(cx)?;
            Ok(vec![])
        }
    }
}

pub fn activate(mut cx: FunctionContext) -> JsResult<JsPromise> {
    validate_gpu(&mut cx)?;

    let (deferred, promise) = cx.promise();
    let channel = cx.channel();

    std::thread::spawn(move || {
        loop{
            // schedule a callback on the node event loop
            let keep_running = channel.send(move |mut cx| {

                // define closure to relay events to js and receive canvas updates in return
                let roundtrip = |payload:Value, windows:Option<&mut WindowManager>| -> NeonResult<()>{
                    let window_state = dispatch_events(&mut cx, payload, windows.is_some())?;
                    if let Some(window_mgr) = windows{
                        for (spec, page) in window_state{
                            window_mgr.update_window(spec, page)
                        }
                    }

                    Ok(())
                };

                // run the winit event loop (either once or until all windows are closed depending on mode)
                Ok(APP.with_borrow_mut(|app| {
                    EVENT_LOOP.with_borrow_mut(|event_loop|{
                        app.activate(event_loop, roundtrip)
                    })
                }))
            }).join();

            // in node-events mode, wait briefly before checking for new events
            match keep_running{
                Ok(true) => sleep(Duration::from_millis(1)),
                _ => break
            }
        }

        // resolve the promise
        deferred.settle_with(&channel, move |mut cx| Ok(cx.undefined()) );
    });

    Ok(promise)
}

pub fn set_rate(mut cx: FunctionContext) -> JsResult<JsNumber> {
    let fps = float_arg(&mut cx, 1, "framesPerSecond")? as u64;
    add_event(AppEvent::FrameRate(fps));
    Ok(cx.number(fps as f64))
}

pub fn set_mode(mut cx: FunctionContext) -> JsResult<JsString> {
    let mode = string_arg(&mut cx, 1, "eventLoopMode")?;
    let loop_mode = match mode.as_str(){
        "node" => Ok(LoopMode::Node),
        "native" => Ok(LoopMode::Native),
        _ => cx.throw_error(format!("Invalid event loop mode: {}", mode))
    }?;
    APP.with_borrow_mut(|app| app.mode = loop_mode );
    Ok(cx.string(mode))
}

pub fn open(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    let win_config = string_arg(&mut cx, 1, "Window configuration")?;
    let context = cx.argument::<BoxedContext2D>(2)?;
    let spec = serde_json::from_str::<WindowSpec>(&win_config).expect("Invalid window state");

    validate_gpu(&mut cx)?;

    add_event(AppEvent::Open(spec, context.borrow().get_page()));
    Ok(cx.undefined())
}

pub fn close(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    let token = float_arg(&mut cx, 1, "windowID")? as u32;
    add_event(AppEvent::Close(token));
    Ok(cx.undefined())
}

pub fn quit(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    APP.with_borrow_mut(|app| app.close_all() );
    add_event(AppEvent::Quit);
    Ok(cx.undefined())
}
