#![allow(unused_mut)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]
use std::sync::OnceLock;
use neon::{prelude::*, result::Throw};
use std::iter::zip;
use serde_json::Value;
use std::cell::RefCell;
use winit::{
    event_loop::{ControlFlow, EventLoop, EventLoopProxy},
    platform::run_on_demand::EventLoopExtRunOnDemand,
};

use crate::utils::*;
use crate::context::BoxedContext2D;
use crate::context::page::Page;

pub mod pump;

pub mod app;
use app::{App, Roundtrip};

pub mod event;
use event::CanvasEvent;

pub mod window;
use window::WindowSpec;

pub mod window_mgr;
use window_mgr::WindowManager;

use crate::gpu::RenderingEngine;

thread_local!(
    static APP_BUNDLE: RefCell<pump::AppBundle> = RefCell::new(pump::AppBundle::default());

    // the event loop can only be run from the main thread
    // static EVENT_LOOP: RefCell<EventLoop<CanvasEvent>> = RefCell::new(EventLoop::with_user_event().build().unwrap());
    // static PROXY: RefCell<EventLoopProxy<CanvasEvent>> = RefCell::new(EVENT_LOOP.with(|event_loop|
    //     event_loop.borrow().create_proxy()
    // ));
);

pub(crate) fn new_proxy() -> EventLoopProxy<CanvasEvent>{
    APP_BUNDLE.with_borrow_mut(|bundle|{
        bundle.app.proxy.clone()
    })
}

pub(crate) fn add_event(event: CanvasEvent){
    APP_BUNDLE.with_borrow_mut(|bundle|{
        bundle.app.proxy.send_event(event).ok()
    });
}

fn validate_gpu(cx:&mut FunctionContext) -> Result<(), Throw>{
    // bail out if we can't draw to the screen
    if let Some(reason) = RenderingEngine::default().lacks_gpu_support(){
        cx.throw_error(reason)?
    }
    Ok(())
}

pub fn launch(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    let callback = cx.argument::<JsFunction>(1)?;

    validate_gpu(&mut cx)?;

    // closure for using the callback to relay events to js and receive updates in return
    let roundtrip = |payload:Value, windows:&mut WindowManager| -> NeonResult<()>{
        let cx = &mut cx;
        let null = cx.null();

        // send payload to js for event dispatch and canvas drawing then read back new state & page data
        let events = cx.string(payload.to_string()).upcast::<JsValue>();
        let response = callback.call(cx, null, vec![events])?
            .downcast::<JsArray, _>(cx).or_throw(cx)?
            .to_vec(cx)?;

        // unpack boxed contexts & window state objects
        let contexts:Vec<Handle<JsValue>> = response[1].downcast::<JsArray, _>(cx).or_throw(cx)?.to_vec(cx)?;
        let specs:Vec<WindowSpec> = serde_json::from_str(
            &response[0].downcast::<JsString, _>(cx).or_throw(cx)?.value(cx)
        ).expect("Malformed response from window event handler");

        // pass each window's new state & page data to the window manager
        zip(contexts, specs).for_each(|(boxed_ctx, spec)| {
            if let Ok(ctx) = boxed_ctx.downcast::<BoxedContext2D, _>(cx){
                windows.update_window(
                    spec.clone(),
                    ctx.borrow().get_page()
                )
            }
        });
        Ok(())
    };

    // EVENT_LOOP.with(|event_loop| {
    //     let mut app = App::with_callback(roundtrip);
    //     let mut event_loop = event_loop.borrow_mut();
    //     event_loop.set_control_flow(ControlFlow::Wait);
    //     event_loop.run_app_on_demand(&mut app)
    // }).ok();

    Ok(cx.undefined())
}


pub fn activate(mut cx: FunctionContext) -> JsResult<JsValue> {
    // validate_gpu(&mut cx)?;

    let payload = APP_BUNDLE.with(|bundle| {
        let mut bundle = bundle.borrow_mut();
        bundle.run_cycle()
    });

    let js_payload = cx.string(payload.to_string()).upcast::<JsValue>();
    Ok(js_payload)
}

pub fn tick(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    let input = cx.argument::<JsArray>(1)?
        .to_vec(&mut cx)?;

    // unpack boxed contexts & window state objects
    let pages:Vec<Page> = input[1].downcast::<JsArray, _>(&mut cx)
        .or_throw(&mut cx)?
        .to_vec(&mut cx)?
        .iter()
        .map(|elt| {
            let boxed_ctx = elt.downcast::<BoxedContext2D, _>(&mut cx).unwrap();
            let ctx = boxed_ctx.borrow();
            ctx.get_page()
        })
        .collect();

    let specs:Vec<WindowSpec> = serde_json::from_str(
        &input[0].downcast::<JsString, _>(&mut cx)
            .or_throw(&mut cx)?
            .value(&mut cx)
    ).expect("Malformed response from window event handler");


    APP_BUNDLE.with(|bundle| {
        let mut bundle = bundle.borrow_mut();
        bundle.app.update_windows(specs, pages);
    });

    // EVENT_LOOP.with(|event_loop| {
    //     let mut event_loop = event_loop.borrow_mut();
    //     APP.with(|app| {
    //         let app = app.borrow_mut();
    //         let timeout = Some(std::time::Duration::ZERO);
    //         let status = event_loop.pump_app_events(timeout, &mut app);
    //     });
    // });
    Ok(cx.undefined())
}

pub fn set_rate(mut cx: FunctionContext) -> JsResult<JsNumber> {
    let fps = float_arg(&mut cx, 1, "framesPerSecond")? as u64;
    add_event(CanvasEvent::FrameRate(fps));
    Ok(cx.number(fps as f64))
}

pub fn open(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    let win_config = string_arg(&mut cx, 0, "Window configuration")?;
    let context = cx.argument::<BoxedContext2D>(1)?;
    let spec = serde_json::from_str::<WindowSpec>(&win_config).expect("Invalid window state");

    validate_gpu(&mut cx)?;

    println!("OPEN");
    APP_BUNDLE.with(|bundle| {
        let mut bundle = bundle.borrow_mut();
        bundle.proxy.send_event(CanvasEvent::Open(spec, context.borrow().get_page())).ok();
    });

    // add_event(CanvasEvent::Open(spec, context.borrow().get_page()));
    Ok(cx.undefined())
}

pub fn close(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    let token = string_arg(&mut cx, 0, "windowID")?;
    add_event(CanvasEvent::Close(token));
    Ok(cx.undefined())
}

pub fn quit(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    add_event(CanvasEvent::Quit);
    Ok(cx.undefined())
}
