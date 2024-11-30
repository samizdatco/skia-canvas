use neon::prelude::*;
use std::time::{Duration, Instant};
use std::iter::zip;
use serde_json::{json, Value};
use winit::{
    application::ApplicationHandler,
    platform::pump_events::{EventLoopExtPumpEvents, PumpStatus},
    platform::run_on_demand::EventLoopExtRunOnDemand,
    event::{ElementState, KeyEvent, StartCause, Event, WindowEvent},
    event_loop::{EventLoop, EventLoopProxy, ActiveEventLoop, ControlFlow},
    keyboard::{PhysicalKey, KeyCode},
    window::WindowId
};

use crate::context::page::Page;
use super::{
    event::CanvasEvent,
    window::WindowSpec,
    window_mgr::WindowManager,
    add_event,
};

pub enum LoopMode{
    Native, Node
}

pub struct App{
    pub mode: LoopMode,
    windows: WindowManager,
    cadence: Cadence,
}

impl Default for App{
    fn default() -> Self {
        Self{
            windows: WindowManager::default(),
            cadence: Cadence::default(),
            mode: LoopMode::Native,
        }
    }
}

#[allow(deprecated)]
impl App{
    pub fn activate<F>(&mut self, event_loop:&mut EventLoop<CanvasEvent>, mut roundtrip:F) -> bool
        where F:FnMut(Value, &mut WindowManager) -> NeonResult<()>
    {
        match self.mode{
            LoopMode::Native => {
                let handler = self.event_handler(roundtrip);
                event_loop.set_control_flow(ControlFlow::Wait);
                event_loop.run_on_demand(handler).ok();
                false
            },
            LoopMode::Node => {
                let handler = self.event_handler(roundtrip);
                match event_loop.pump_events(Some(std::time::Duration::ZERO), handler){
                    PumpStatus::Exit(..) => false,
                    PumpStatus::Continue => true,
                }
            }
        }
    }

    pub fn close_all(&mut self){
        self.windows.remove_all();
    }

    pub fn event_handler<F>(&mut self, mut roundtrip:F) -> impl FnMut(Event<CanvasEvent>, &ActiveEventLoop) + use<'_, F>
        where F:FnMut(Value, &mut WindowManager) -> NeonResult<()>
    {
        move |event, event_loop| match event {
            Event::WindowEvent { event:ref win_event, window_id } => {
                self.windows.capture_ui_event(&window_id, win_event);

                match win_event {
                    WindowEvent::Destroyed | WindowEvent::CloseRequested => {
                        self.windows.remove(&window_id);

                        // exit after the last window is closed (but only in run_on_demand mode)
                        if self.windows.is_empty() && matches!(self.mode, LoopMode::Native){
                            add_event(CanvasEvent::Quit);
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
                        self.windows.suspend_redraw(&window_id, *is_hidden);
                    }

                    WindowEvent::RedrawRequested => {
                        self.windows.redraw(&window_id);
                    }

                    WindowEvent::Resized(size) => {
                        self.windows.resized(&window_id, *size);
                    }

                    _ => {}
                }
            },


            Event::UserEvent(canvas_event) => match canvas_event{
                CanvasEvent::Open(spec, page) => {
                    self.windows.add(event_loop, spec, page);
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
                    roundtrip(self.windows.get_ui_changes(), &mut self.windows).ok();
                }
                CanvasEvent::FrameRate(fps) => {
                    self.cadence.set_frame_rate(fps)
                }
            },


            Event::AboutToWait => {
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
        }
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

