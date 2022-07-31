#![allow(unused_mut)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]
use neon::prelude::*;
use serde_json::json;
use std::cell::RefCell;
use winit::{
    event_loop::{ControlFlow, EventLoop, EventLoopProxy},
    event::{Event, WindowEvent, KeyboardInput, VirtualKeyCode, ElementState},
    platform::run_return::EventLoopExtRunReturn
};

use crate::utils::*;
use crate::gpu::runloop;
use crate::context::{BoxedContext2D, page::Page};

pub mod event;
use event::{Cadence, CanvasEvent};

pub mod window;
use window::{Window, WindowSpec, WindowManager};

thread_local!(
    // the event loop can only be run from the main thread
    static EVENT_LOOP: RefCell<EventLoop<CanvasEvent>> = RefCell::new(EventLoop::with_user_event());
    static PROXY: RefCell<EventLoopProxy<CanvasEvent>> = RefCell::new(EVENT_LOOP.with(|event_loop|
        event_loop.borrow().create_proxy()
    ));
);

fn new_proxy() -> EventLoopProxy<CanvasEvent>{
    PROXY.with(|cell| cell.borrow().clone() )
}

fn add_event(event: CanvasEvent){
    PROXY.with(|cell| cell.borrow().send_event(event).ok() );
}

fn roundtrip<'a, F>(cx: &'a mut FunctionContext, payload:serde_json::Value, callback:&Handle<JsFunction>, mut f:F) -> NeonResult<()>
    where F:FnMut(WindowSpec, Page)
{
    let null = cx.null();
    let events = cx.string(payload.to_string()).upcast::<JsValue>();

    let response = callback.call(cx, null, vec![events]).expect("Error in Window event handler")
        .downcast::<JsArray, _>(cx).or_throw(cx)?
        .to_vec(cx)?;
    let specs:Vec<WindowSpec> = serde_json::from_str(
        &response[0].downcast::<JsString, _>(cx).or_throw(cx)?.value(cx)
    ).unwrap();

    response[1].downcast::<JsArray, _>(cx).or_throw(cx)?.to_vec(cx)?
        .iter()
        .map(|obj| obj.downcast::<BoxedContext2D, _>(cx))
        .filter( |ctx| ctx.is_ok() )
        .zip(specs.iter())
        .for_each(|(ctx, spec)| {
            f(spec.clone(), ctx.unwrap().borrow().get_page());
        });
    Ok(())
}

pub fn launch(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    let callback = cx.argument::<JsFunction>(1)?;

    let mut windows = WindowManager::default();
    let mut cadence = Cadence::default();
    let mut frame:u64 = 0;

    cadence.set_frame_rate(60);

    EVENT_LOOP.with(|event_loop| {
        event_loop.borrow_mut().run_return(|event, event_loop, control_flow| {
            runloop(|| {
                match event {
                    Event::NewEvents(..) => {
                        *control_flow = cadence.on_next_frame(|| add_event(CanvasEvent::Render) );
                    }

                    Event::UserEvent(canvas_event) => {
                        match canvas_event{
                            CanvasEvent::Open(spec, page) => {
                                windows.add(event_loop, new_proxy(), spec, page);
                            }
                            CanvasEvent::Close(token) => {
                                windows.remove_by_token(&token);
                            }
                            CanvasEvent::Quit => {
                                return *control_flow = ControlFlow::Exit;
                            }
                            CanvasEvent::Render => {
                                // on initial pass, do a roundtrip to sync up the Window object's state attrs:
                                // send just the initial window positions then read back all state
                                cadence.on_startup(||{
                                    roundtrip(&mut cx, json!({"geom":windows.get_geometry()}), &callback,
                                        |spec, page| windows.update_window(spec, page)
                                    ).ok();
                                });

                                // relay UI-driven state changes to js and render the response
                                frame += 1;
                                let payload = json!{{
                                    "frame": frame,
                                    "changes": windows.get_ui_changes(),
                                    "state": windows.get_state(),
                                }};
                                roundtrip(&mut cx, payload, &callback,
                                    |spec, page| windows.update_window(spec, page)
                                ).ok();
                            }
                            CanvasEvent::Transform(window_id, matrix) => {
                                windows.use_ui_transform(&window_id, &matrix);
                            },
                            CanvasEvent::InFullscreen(window_id, is_fullscreen) => {
                                windows.use_fullscreen_state(&window_id, is_fullscreen);
                            }
                            CanvasEvent::FrameRate(fps) => {
                                cadence.set_frame_rate(fps)
                            }
                            _ => {}
                        }
                    }

                    Event::WindowEvent { event:ref win_event, window_id } => match win_event {
                        WindowEvent::Destroyed | WindowEvent::CloseRequested => {
                            windows.remove(&window_id);
                        }
                        WindowEvent::KeyboardInput { input: KeyboardInput { virtual_keycode: Some(VirtualKeyCode::Escape), state: ElementState::Released,.. }, .. } => {
                            windows.set_fullscreen_state(&window_id, false);
                        }
                        WindowEvent::Resized(_) => {
                            windows.capture_ui_event(&window_id, win_event); // update state
                            windows.send_event(&window_id, event); // update the window
                        }
                        _ => {
                            windows.capture_ui_event(&window_id, win_event);
                        }
                    },

                    Event::RedrawRequested(window_id) => {
                        windows.send_event(&window_id, event);
                    }

                    Event::RedrawEventsCleared => {
                        *control_flow = match windows.len(){
                            0 => ControlFlow::Exit,
                            _ => ControlFlow::Poll
                        }
                    }

                    _ => {}
                }
            });
        });
    });

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
    add_event(CanvasEvent::Open(spec, context.borrow().get_page()));
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
