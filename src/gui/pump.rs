use neon::prelude::*;
use std::time::{Duration, Instant};
use std::iter::zip;
use serde_json::{json, Value};
use winit::{
    application::ApplicationHandler,
    platform::pump_events::{EventLoopExtPumpEvents, PumpStatus},
    event::{ElementState, KeyEvent, StartCause, Event, WindowEvent},
    event_loop::{EventLoop, EventLoopProxy, ActiveEventLoop, ControlFlow},
    keyboard::{PhysicalKey, KeyCode},
    window::WindowId
};

use super::event::CanvasEvent;
use super::window::WindowSpec;
use super::window_mgr::WindowManager;
use super::{add_event, new_proxy};
use crate::context::page::Page;

pub struct AppBundle{
    windows: WindowManager,
    cadence: Cadence,
}

impl Default for AppBundle{
    fn default() -> Self {
        let windows = WindowManager::default();
        let cadence = Cadence::default();
        Self{windows, cadence}
    }
}

#[allow(deprecated)]
impl AppBundle{
    pub fn run_cycle<F>(&mut self, event_loop:&mut EventLoop<CanvasEvent>, mut roundtrip:F) -> Value
        where F:FnMut(Value, &mut WindowManager) -> NeonResult<()>
    {
        let mut payload = json!({});

        let timeout = Some(Duration::ZERO);
        let status = event_loop.pump_events(timeout, |event, event_loop| match event {
            Event::WindowEvent { event:ref win_event, window_id } => {
                self.windows.capture_ui_event(&window_id, win_event);

                match win_event {
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
                        self.windows.send_event(&window_id, CanvasEvent::RedrawingSuspended(*is_hidden));
                    }

                    WindowEvent::RedrawRequested => {
                        self.windows.send_event(&window_id, CanvasEvent::RedrawRequested);
                    }

                    WindowEvent::Resized(size) => {
                        self.windows.send_event(&window_id, CanvasEvent::WindowResized(*size));
                    }

                    _ => {}
                }
            },


            Event::UserEvent(canvas_event) => match canvas_event{
                CanvasEvent::Open(spec, page) => {
                    self.windows.add(event_loop, new_proxy(), spec, page);
                    // self.initial_sync();
                    // payload = self.windows.get_geometry();
                    roundtrip(self.windows.get_geometry(), &mut self.windows).ok();
                }
                CanvasEvent::Close(token) => {
                    self.windows.remove_by_token(&token);
                }
                CanvasEvent::Quit => {
                    event_loop.exit();
                }
                CanvasEvent::Render => {
                    // relay UI-driven state changes to js and render the next frame in the (active) cadence
                    // self.roundtrip();
                    // payload = self.windows.get_ui_changes();
                    roundtrip(self.windows.get_ui_changes(), &mut self.windows).ok();
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
            },


            Event::AboutToWait => {
                self.windows.dispatch_events();

                // when no windows have frame/draw handlers, the (inactive) cadence will never trigger
                // a Render event, so only do a roundtrip if there are new UI events to be relayed
                if !self.cadence.active() && self.windows.has_ui_changes() {
                    // payload = self.windows.get_ui_changes();
                    roundtrip(self.windows.get_ui_changes(), &mut self.windows).ok();
                }

                // delegate timing to the cadence if active, otherwise wait for ui events
                event_loop.set_control_flow(
                    match self.cadence.active(){
                        true => self.cadence.on_next_frame(||{ add_event(CanvasEvent::Render) }),
                        false => ControlFlow::Wait
                    }
                );

            }
            _ => {}
        });

        payload
    }

    pub fn update_windows(&mut self, specs:Vec<WindowSpec>, pages:Vec<Page>){
        zip(specs, pages).for_each(|(spec, page)| {
            self.windows.update_window(spec, page)
        });
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

