#![allow(unused_mut)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]
use neon::prelude::*;
use serde_json::json;
use crossbeam::channel::{self, Sender, Receiver};
use std::{
    cell::RefCell,
    collections::HashMap,
    thread, borrow::BorrowMut
};
use winit::{
    dpi::{LogicalSize, LogicalPosition, PhysicalSize, PhysicalPosition, Position},
    event_loop::{ControlFlow, EventLoop, EventLoopProxy},
    event::{Event, WindowEvent, ElementState,  KeyboardInput, VirtualKeyCode, ModifiersState},
    window::{WindowId},
    platform::run_return::EventLoopExtRunReturn
};

use crate::utils::*;
use crate::gpu::runloop;
use crate::context::{BoxedContext2D, page::Page};

pub mod event;
use event::{Cadence, CanvasEvent};

pub mod window;
use window::{Window, WindowSpec};

thread_local!(
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

fn send_event_to(windows:&HashMap<WindowId, Sender<Event<'static, CanvasEvent>>>, id:&WindowId, event:Event<CanvasEvent>){
    if let Some(tx) = windows.get(id) {
        if let Some(event) = event.to_static() {
            tx.send(event).ok();
        }
    }
}

fn roundtrip<'a, F>(cx: &'a mut FunctionContext, payload:serde_json::Value, callback:&Handle<JsFunction>, mut f:F) -> NeonResult<()>
    where F:FnMut(&WindowSpec, Page)
{
    let null = cx.null();
    let idents = cx.string(payload.to_string()).upcast::<JsValue>();
    let response = callback.call(cx, null, vec![idents]).expect("Error in Window event handler")
        .downcast::<JsArray, _>(cx).or_throw(cx)?
        .to_vec(cx)?;
    let specs:Vec<WindowSpec> = serde_json::from_str(
        &response[0].downcast::<JsString, _>(cx).or_throw(cx)?.value(cx)
    ).unwrap();
    let contexts = response[1].downcast::<JsArray, _>(cx).or_throw(cx)?.to_vec(cx)?
        .iter()
        .map(|obj| obj.downcast::<BoxedContext2D, _>(cx))
        .filter( |ctx| ctx.is_ok() )
        .zip(specs.iter())
        .for_each(|(ctx, spec)| {
            f(&spec, ctx.unwrap().borrow().get_page());
        });
    Ok(())
}

pub fn launch(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    let callback = cx.argument::<JsFunction>(1)?;

    let mut offset:LogicalPosition<i32> = LogicalPosition::new(0, 0);
    let mut windows: HashMap<WindowId, Sender<Event<'static, CanvasEvent>>> = HashMap::default();
    let mut window_ids: HashMap<String, WindowId> = HashMap::default();
    let mut cadence = Cadence::default();
    let mut frame:u64 = 0;

    cadence.set_frame_rate(60);

    EVENT_LOOP.with(|mut event_loop| {
        event_loop.borrow_mut().run_return(|event, event_loop, control_flow| {
            runloop(|| {
                match event {
                    Event::NewEvents(..) => {
                        *control_flow = cadence.on_next_frame(||
                            add_event(match windows.len(){
                                0 => CanvasEvent::Quit,
                                _ => CanvasEvent::Render
                            })
                        )
                    }

                    Event::UserEvent(ref canvas_event) => {
                        match canvas_event{
                            CanvasEvent::Open(spec, page) => {
                                let mut spec = spec.clone();
                                spec.x = offset.x;
                                spec.y = offset.y;
                                offset.x += 30;
                                offset.y += 30;
                                let mut window = Window::new(event_loop, new_proxy(), &spec, page.clone());
                                let id = window.handle.id();
                                let (tx, rx) = channel::bounded(50);

                                window_ids.insert(spec.id.clone(), id);
                                windows.insert(id, tx);

                                thread::spawn(move || {
                                    while let Ok(event) = rx.recv() {
                                        window.handle_event(event);
                                    }
                                });
                            }
                            CanvasEvent::Close(token) => {
                                if let Some(window_id) = window_ids.get(token){
                                    windows.remove(&window_id);
                                }
                            }
                            CanvasEvent::Quit => {
                                return *control_flow = ControlFlow::Exit;
                            }
                            CanvasEvent::Render => {
                                frame += 1;
                                roundtrip(&mut cx, json!({ "frame": frame }), &callback, |spec, page| {
                                    if let Some(window_id) = window_ids.get(&spec.id){
                                        send_event_to(&windows, window_id, Event::UserEvent(CanvasEvent::Page(page)))
                                    }
                                }).ok();
                            }
                            _ => {}
                        //   CanvasEvent::Heartbeat => window.autohide_cursor(),
                        //   CanvasEvent::FrameRate(fps) => cadence.set_frame_rate(fps),
                        //   CanvasEvent::InFullscreen(to_full) => window.went_fullscreen(to_full),
                        //   CanvasEvent::Transform(matrix) => window.new_transform(matrix),
                        //   _ => window.send_js_event(canvas_event)
                        }
                    }

                    Event::WindowEvent { event:ref win_event, window_id } => match win_event {
                        #[allow(deprecated)]
                        WindowEvent::Destroyed |
                        WindowEvent::CloseRequested |
                        WindowEvent::KeyboardInput { input: KeyboardInput { virtual_keycode: Some(VirtualKeyCode::Escape), state: ElementState::Released,.. }, .. } |
                        WindowEvent::KeyboardInput {
                            input: KeyboardInput {
                                state: ElementState::Released,
                                virtual_keycode: Some(VirtualKeyCode::W),
                                modifiers: ModifiersState::LOGO, ..
                            }, ..
                        } => {
                            windows.remove(&window_id);
                        }
                        _ => {
                            send_event_to(&windows, &window_id, event);
                        }
                    },

                    Event::RedrawRequested(window_id) => {
                        send_event_to(&windows, &window_id, event);
                    }

                    _ => {}
                }
            });
        });
    });

    Ok(cx.undefined())
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
