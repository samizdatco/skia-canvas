use neon::prelude::*;
use serde_json::Value;
use std::{
    sync::{Arc, OnceLock},
    iter::zip,
    cell::RefCell,
    thread::sleep,
    time::{Duration, Instant},
};
use winit::{
    platform::pump_events::EventLoopExtPumpEvents,
    platform::run_on_demand::EventLoopExtRunOnDemand,
    event::{ElementState, KeyEvent, Event, WindowEvent},
    event_loop::{EventLoop, EventLoopProxy, ActiveEventLoop, ControlFlow},
    keyboard::{PhysicalKey, KeyCode},
};

use crate::context::{page::Page, BoxedContext2D};
use super::{
    event::AppEvent,
    window_mgr::WindowManager,
    window::WindowSpec,
};

thread_local!(
    static APP: RefCell<App> = RefCell::new(App::default());
    static EVENT_LOOP: RefCell<EventLoop<AppEvent>> = RefCell::new(EventLoop::with_user_event().build().unwrap());
    static PROXY: RefCell<EventLoopProxy<AppEvent>> = RefCell::new(EVENT_LOOP.with_borrow(|event_loop|
        event_loop.create_proxy()
    ));
);

static RENDER_CALLBACK: OnceLock<Arc<Root<JsFunction>>> = OnceLock::new();

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

fn add_event(event: AppEvent){
    PROXY.with_borrow_mut(|proxy| proxy.send_event(event).ok() );
}

impl App{
    pub fn register(callback:Root<JsFunction>){
        RENDER_CALLBACK.get_or_init(|| Arc::new(callback));
    }

    pub fn set_mode(mode:LoopMode){
        APP.with_borrow_mut(|app| app.mode = mode );
    }

    pub fn set_fps(fps:f32){
        add_event(AppEvent::FrameRate(fps as u64));
    }

    pub fn open_window(spec:WindowSpec, page:Page){
        add_event(AppEvent::Open(spec, page));
    }

    pub fn close_window(token:u32){
        add_event(AppEvent::Close(token));
    }

    pub fn quit(){
        APP.with_borrow_mut(|app| app.windows.remove_all() );
        add_event(AppEvent::Quit);
    }

    #[allow(deprecated)]
    pub fn activate(channel:Channel, deferred:neon::types::Deferred){
        std::thread::spawn(move || {
            loop{
                // schedule a callback on the node event loop
                let keep_running = channel.send(move |mut cx| {

                    // define closure to relay events to js and receive canvas updates in return
                    let roundtrip = |payload:Value, windows:Option<&mut WindowManager>| -> NeonResult<()>{
                        let window_state = App::dispatch_events(&mut cx, payload, windows.is_some())?;
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
                            match app.mode{
                                LoopMode::Native => {
                                    let handler = app.event_handler(roundtrip);
                                    event_loop.set_control_flow(ControlFlow::Wait);
                                    event_loop.run_on_demand(handler).ok();
                                    false
                                },
                                LoopMode::Node => {
                                    let handler = app.event_handler(roundtrip);
                                    event_loop.pump_events(Some(Duration::ZERO), handler);
                                    app.cadence.should_continue() || !app.windows.is_empty()
                                }
                            }
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
                    .or_else(|err| cx.throw_error(format!("Malformed response from window event handler: {}", err)) )?;

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

    fn event_handler<F>(&mut self, mut roundtrip:F) -> impl FnMut(Event<AppEvent>, &ActiveEventLoop) + use<'_, F>
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
    needs_cleanup: Option<bool>,
}

impl Default for Cadence {
    fn default() -> Self {
        Self{
            rate: 60,
            last: Instant::now(),
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
        self.rate = rate;
    }

    pub fn on_next_frame<F:FnMut()>(&mut self, mode:LoopMode, mut draw:F) -> ControlFlow{
        // determine the upcoming deadlines for actually rendering and for spinning in preparation
        let frame_time = 1_000_000_000/self.rate.max(1);
        let watch_interval = 1_500_000.min(frame_time/10);
        let render = Duration::from_nanos(frame_time);
        let wakeup = Duration::from_nanos(frame_time - watch_interval);

        // if node is handling the event loop, we can't use polling to wait for the render
        // deadline. so instead we'll pause the thread for the last 10% of the inter-frame
        // time (up to 1.5ms), making sure we can then draw immediately after
        let dt = self.last.elapsed();
        if matches!(mode, LoopMode::Node) && dt >= wakeup && dt < render{
            if let Some(sleep_time) = render.checked_sub(self.last.elapsed()){
                spin_sleep::sleep(sleep_time);
            }
        }

        // call the draw callback if it's time & make sure the next deadline is in the future
        if self.last.elapsed() >= render{
            draw();
            while self.last < Instant::now() - render{
                self.last += render
            }
        }

        // if winit is in control, we can use waiting & polling to hit the deadline
        match self.last.elapsed() < wakeup {
            true => ControlFlow::WaitUntil(self.last + wakeup),
            false => ControlFlow::Poll,
        }
    }
}
