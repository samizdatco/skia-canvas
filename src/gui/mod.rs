#![allow(unused_mut)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]
use neon::prelude::*;
use std::iter::zip;
use serde_json::{json, Value};
use std::cell::RefCell;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, Event, KeyEvent, StartCause, WindowEvent::{self, KeyboardInput}}, 
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy}, 
    keyboard::{PhysicalKey, NamedKey, KeyCode}, 
    platform::run_on_demand::EventLoopExtRunOnDemand,
    window::WindowId
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
    static EVENT_LOOP: RefCell<EventLoop<CanvasEvent>> = RefCell::new(EventLoop::with_user_event().build().unwrap());
    static PROXY: RefCell<EventLoopProxy<CanvasEvent>> = RefCell::new(EVENT_LOOP.with(|event_loop|
        event_loop.borrow().create_proxy()
    ));
);

trait Roundtrip: FnMut(Value, &mut WindowManager) -> NeonResult<()>{}
impl<T:FnMut(Value, &mut WindowManager) -> NeonResult<()>> Roundtrip for T {}

struct App<F:Roundtrip>{
    windows: WindowManager,
    cadence: Cadence,
    callback: F
}

impl<F:Roundtrip> App<F>{
    fn with_callback(callback:F) -> Self{
        let windows = WindowManager::default();
        let cadence = Cadence::default();
        Self{windows, cadence, callback}
    }

    fn initial_sync(&mut self){
        let payload = self.windows.get_geometry();
        let _ = (self.callback)(payload, &mut self.windows);
    }

    fn roundtrip(&mut self){
        let payload = self.windows.get_ui_changes();
        let _ = (self.callback)(payload, &mut self.windows);
    }
}

impl<F:Roundtrip> ApplicationHandler<CanvasEvent> for App<F> {
    fn resumed(&mut self, event_loop:&ActiveEventLoop){

    }

    fn new_events(&mut self, event_loop:&ActiveEventLoop, cause:StartCause) { 
        // trigger a Render if the cadence is active, otherwise handle UI events in MainEventsCleared
        event_loop.set_control_flow(
            self.cadence.on_next_frame(|| add_event(CanvasEvent::Render) )
        );
    }

    fn window_event( &mut self, event_loop:&ActiveEventLoop, window_id:WindowId, event:WindowEvent){
        // route UI events to the relevant window
        self.windows.capture_ui_event(&window_id, &event);

        // handle window lifecycle events
        match event {
            WindowEvent::Destroyed | WindowEvent::CloseRequested => {
                self.windows.remove(&window_id);
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(KeyCode::Escape),
                        state: ElementState::Pressed,
                        repeat: false,
                        ..
                    },
                ..
            } => {
                self.windows.set_fullscreen_state(&window_id, false);
            }

            WindowEvent::RedrawRequested => {
                self.windows.send_event(&window_id, CanvasEvent::RedrawRequested);
            }

            WindowEvent::Resized(size) => {
                self.windows.send_event(&window_id, CanvasEvent::WindowResized(size));
            }
            _ => {}
        }
    }

    fn user_event(&mut self, event_loop:&ActiveEventLoop, event:CanvasEvent) { 
        match event{
            CanvasEvent::Open(spec, page) => {
                self.windows.add(event_loop, new_proxy(), spec, page);
            }
            CanvasEvent::Close(token) => {
                self.windows.remove_by_token(&token);
            }
            CanvasEvent::Quit => {
                event_loop.exit();
            }
            CanvasEvent::Render => {
                // relay UI-driven state changes to js and render the next frame in the (active) cadence
                self.roundtrip();
            }
            CanvasEvent::Transform(window_id, matrix) => {
                self.windows.use_ui_transform(&window_id, &matrix);
            },
            CanvasEvent::InFullscreen(window_id, is_fullscreen) => {
                self.windows.use_fullscreen_state(&window_id, is_fullscreen);
            }
            CanvasEvent::FrameRate(fps) => {
                self.cadence.set_frame_rate(fps)
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop:&ActiveEventLoop) { 
        // on initial pass, do a roundtrip to sync up the Window object's state attrs:
        // send just the initial window positions then read back all state
        if self.cadence.at_startup(){
            self.initial_sync();
        }
        
        // when no windows have frame/draw handlers, the (inactive) cadence will never trigger
        // a Render event, so only do a roundtrip if there are new UI events to be relayed
        if !self.cadence.active() && self.windows.has_ui_changes() {
            self.roundtrip();
        }

        // quit after the last window is closed
        match (self.windows.len(), self.cadence.active()) {
            (0, _) => event_loop.exit(),
            (_, false) => event_loop.set_control_flow(ControlFlow::Wait),
            _ => event_loop.set_control_flow(ControlFlow::Poll)
        };
    }

}


fn new_proxy() -> EventLoopProxy<CanvasEvent>{
    PROXY.with(|cell| cell.borrow().clone() )
}

fn add_event(event: CanvasEvent){
    PROXY.with(|cell| cell.borrow().send_event(event).ok() );
}

pub fn launch(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    let callback = cx.argument::<JsFunction>(1)?;

    // closure for using the callback to relay events to js and receive updates in return
    let roundtrip = |payload:Value, windows:&mut WindowManager| -> NeonResult<()>{
        let mut cx = &mut cx;
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

    EVENT_LOOP.with(|event_loop| {
        let mut app = App::with_callback(roundtrip);
        let mut event_loop = event_loop.borrow_mut();
        event_loop.set_control_flow(ControlFlow::Wait);
        event_loop.run_app_on_demand(&mut app)
    }).ok();

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
