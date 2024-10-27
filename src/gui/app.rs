use neon::prelude::*;
use std::time::{Duration, Instant};
use serde_json::Value;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, StartCause, WindowEvent}, 
    event_loop::{ActiveEventLoop, ControlFlow}, 
    keyboard::{PhysicalKey, KeyCode}, 
    window::WindowId
};

use super::event::CanvasEvent;
use super::window_mgr::WindowManager;
use super::{add_event, new_proxy};

pub trait Roundtrip: FnMut(Value, &mut WindowManager) -> NeonResult<()>{}
impl<T:FnMut(Value, &mut WindowManager) -> NeonResult<()>> Roundtrip for T {}

pub struct App<F:Roundtrip>{
    windows: WindowManager,
    cadence: Cadence,
    callback: F,
}

impl<F:Roundtrip> App<F>{
    pub fn with_callback(callback:F) -> Self{
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
        if cause == StartCause::Init{
            // on initial pass, do a roundtrip to sync up the Window object's state attrs:
            // send just the initial window positions then read back all state
            self.initial_sync();
        }
    }

    fn window_event( &mut self, event_loop:&ActiveEventLoop, window_id:WindowId, event:WindowEvent){
        // route UI events to the relevant window
        self.windows.capture_ui_event(&window_id, &event);

        // handle window lifecycle events
        match event {
            WindowEvent::Destroyed | WindowEvent::CloseRequested => {
                self.windows.remove(&window_id);                
                if self.windows.is_empty() {
                    // quit after the last window is closed
                    event_loop.exit(); 
                }
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

            #[cfg(target_os = "macos")]
            WindowEvent::Occluded(is_hidden) => {
                self.windows.send_event(&window_id, CanvasEvent::RedrawingSuspended(is_hidden));
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
        // when no windows have frame/draw handlers, the (inactive) cadence will never trigger
        // a Render event, so only do a roundtrip if there are new UI events to be relayed
        if !self.cadence.active() && self.windows.has_ui_changes() {
            self.roundtrip();
        }

        // delegate timing to the cadence if active, otherwise wait for ui events
        event_loop.set_control_flow( 
            match self.cadence.active(){
                true => self.cadence.on_next_frame(|| add_event(CanvasEvent::Render) ),
                false => ControlFlow::Wait
            }
        );
    }

}


struct Cadence{
    rate: u64,
    last: Instant,
    interval: Duration,
    begun: bool,
}

impl Default for Cadence {
    fn default() -> Self {
        Self{
            rate: 0,
            last: Instant::now(),
            interval: Duration::new(0, 0),
            begun: false,
        }
    }
}

impl Cadence{
    fn at_startup(&mut self) -> bool{
        if self.begun{ false }
        else{ 
            self.begun = true;
            true // only return true on first call
        }
    }

    fn set_frame_rate(&mut self, rate:u64){
        if rate == self.rate{ return }
        let frame_time = 1_000_000_000/rate.max(1);
        self.interval = Duration::from_nanos(frame_time);
        self.rate = rate;
    }

    fn on_next_frame<F:Fn()>(&mut self, draw:F) -> ControlFlow{
        match self.active() {
            true => {
                if self.last.elapsed() >= self.interval{
                    while self.last < Instant::now() - self.interval{
                        self.last += self.interval
                    }
                    draw();
                }        
                ControlFlow::WaitUntil(self.last + self.interval)
            },
            false => ControlFlow::Wait,
        }
    }

    fn active(&self) -> bool{
        self.rate > 0
    }
}

  