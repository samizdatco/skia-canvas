use neon::prelude::*;
use std::time::{Duration, Instant};
use serde_json::Value;
use winit::{
    platform::pump_events::{EventLoopExtPumpEvents},
    platform::run_on_demand::EventLoopExtRunOnDemand,
    event::{ElementState, KeyEvent, Event, WindowEvent},
    event_loop::{EventLoop, ActiveEventLoop, ControlFlow},
    keyboard::{PhysicalKey, KeyCode},
};

use super::{
    event::AppEvent,
    window_mgr::WindowManager,
};

#[derive(Copy, Clone)]
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
    pub fn activate<F>(&mut self, event_loop:&mut EventLoop<AppEvent>, roundtrip:F) -> bool
        where F:FnMut(Value, Option<&mut WindowManager>) -> NeonResult<()>
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
                event_loop.pump_events(Some(Duration::ZERO), handler);
                self.cadence.should_continue() || !self.windows.is_empty()
            }
        }
    }

    pub fn close_all(&mut self){
        self.windows.remove_all();
    }

    pub fn event_handler<F>(&mut self, mut roundtrip:F) -> impl FnMut(Event<AppEvent>, &ActiveEventLoop) + use<'_, F>
        where F:FnMut(Value, Option<&mut WindowManager>) -> NeonResult<()>
    {
        move |event, event_loop| match event {
            Event::WindowEvent { event:ref win_event, window_id } => {
                self.windows.capture_ui_event(&window_id, win_event);

                match win_event {
                    WindowEvent::Destroyed | WindowEvent::CloseRequested => {
                        self.windows.remove(&window_id);

                        // after the last window is closed, either exit (in run_on_demand mode)
                        // or wait for the window destructor to run (in pump_events mode)
                        if self.windows.is_empty(){ match self.mode{
                            LoopMode::Native => event_loop.exit(),
                            LoopMode::Node => self.cadence.loop_again(),
                        }}
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
                        self.windows.find(&window_id, |win| win.set_fullscreen(false) );
                    }

                    WindowEvent::Moved(loc) => {
                        self.windows.find(&window_id, |win| win.did_move(*loc) );
                    }

                    WindowEvent::Resized(size) => {
                        self.windows.find(&window_id, |win| win.did_resize(*size) );
                    }

                    #[cfg(target_os = "macos")]
                    WindowEvent::Occluded(is_hidden) => {
                        self.windows.find(&window_id, |win| win.set_redrawing_suspended(*is_hidden) );
                    }

                    WindowEvent::RedrawRequested => {
                        self.windows.find(&window_id, |win| win.redraw() );
                    }

                    _ => {}
                }
            },


            Event::UserEvent(app_event) => match app_event{
                AppEvent::Open(spec, page) => {
                    self.windows.add(event_loop, spec, page);
                    roundtrip(self.windows.get_geometry(), Some(&mut self.windows)).ok();
                }
                AppEvent::Close(token) => {
                    self.windows.remove_by_token(token);
                }
                AppEvent::FrameRate(fps) => {
                    self.cadence.set_frame_rate(fps)
                }
                AppEvent::Quit => {
                    event_loop.exit();
                }
            },


            Event::AboutToWait => {
                // dispatch UI events if new ones have arrived
                if self.windows.has_ui_changes() {
                    roundtrip(self.windows.get_ui_changes(), None).ok();
                }

                // let the cadence decide when to switch to poll-mode or sleep the thread
                event_loop.set_control_flow(
                    self.cadence.on_next_frame(self.mode, || {
                        // relay UI-driven state changes to js and render the next frame in the (active) cadence
                        roundtrip(self.windows.get_ui_changes(), Some(&mut self.windows)).ok();
                    })
                );
            }
            _ => {}
        }
    }
}


struct Cadence{
    rate: u64,
    last: Instant,
    wakeup: Duration,
    render: Duration,
    needs_cleanup: Option<bool>,
}

impl Default for Cadence {
    fn default() -> Self {
        Self{
            rate: 0,
            last: Instant::now(),
            wakeup: Duration::new(0, 0),
            render: Duration::new(0, 0),
            needs_cleanup: Some(true), // ensure at least one post-Init loop
        }
    }
}

impl Cadence{
    fn loop_again(&mut self){
        // flag that a clean-up event-loop pass is necessary (e.g., for reflecting window closures)
        self.needs_cleanup = Some(true)
    }

    fn should_continue(&mut self) -> bool{
        self.needs_cleanup.take().is_some()
    }

    fn set_frame_rate(&mut self, rate:u64){
        if rate == self.rate{ return }
        let frame_time = 1_000_000_000/rate.max(1);
        let watch_interval = 1_500_000.max(frame_time/10);
        self.render = Duration::from_nanos(frame_time);
        self.wakeup = Duration::from_nanos(frame_time - watch_interval);
        self.rate = rate;
    }

    pub fn on_next_frame<F:FnMut()>(&mut self, mode:LoopMode, mut draw:F) -> ControlFlow{
        // if node is handling the event loop, we can't use polling to wait for the
        // render deadline, so instead we'll pause the thread for up to 1.5ms, making sure
        // we can then draw immediately after
        let dt = self.last.elapsed();
        if matches!(mode, LoopMode::Node) && dt >= self.wakeup && dt < self.render{
            if let Some(sleep_time) = self.render.checked_sub(self.last.elapsed()){
                spin_sleep::sleep(sleep_time);
            }
        }

        // call the draw callback if it's time & make sure the next deadline is in the future
        if self.last.elapsed() >= self.render{
            draw();
            while self.last < Instant::now() - self.render{
                self.last += self.render
            }
        }

        // if winit is in control, we can use waiting & polling to hit the deadline
        match self.last.elapsed() < self.wakeup {
            true => ControlFlow::WaitUntil(self.last + self.wakeup),
            false => ControlFlow::Poll,
        }
    }
}

