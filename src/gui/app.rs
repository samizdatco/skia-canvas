use neon::prelude::*;
use serde_json::Value;
use std::{
    sync::{Arc, OnceLock, atomic::{AtomicBool, Ordering}},
    iter::zip,
    cell::RefCell,
    time::{Duration, Instant},
};
use winit::{
    application::ApplicationHandler,
    platform::pump_events::EventLoopExtPumpEvents,
    platform::run_on_demand::EventLoopExtRunOnDemand,
    event::{ElementState, KeyEvent, WindowEvent},
    event_loop::{EventLoop, EventLoopProxy, ActiveEventLoop, ControlFlow},
    keyboard::{PhysicalKey, KeyCode},
    window::WindowId,
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
    vsync: Option<VsyncSource>,
}

impl Default for App{
    fn default() -> Self {
        Self{
            windows: WindowManager::default(),
            cadence: Cadence::default(),
            mode: LoopMode::Native,
            vsync: None,
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

    pub fn activate(channel:Channel, deferred:neon::types::Deferred){
        std::thread::spawn(move || {
            loop{
                // schedule a callback on the node event loop
                let keep_running = channel.send(move |mut cx| {

                    // define closure to relay events to js and receive canvas updates in return
                    let dispatch = |payload:Value, windows:Option<&mut WindowManager>| -> NeonResult<()>{
                        App::dispatch_events(&mut cx, payload, windows)
                    };

                    // run the winit event loop (either once or until all windows are closed depending on mode)
                    APP.with_borrow_mut(|app| {
                        EVENT_LOOP.with_borrow_mut(|event_loop|{
                            match app.mode{
                                LoopMode::Native => {
                                    event_loop.set_control_flow(ControlFlow::Wait);
                                    event_loop.run_app_on_demand(&mut AppHandler{app, dispatch}).ok();
                                    Ok(false) // final window was closed
                                }
                                LoopMode::Node => {
                                    // wait for events to arrive (unless interrupted by a vsync tick or `backstop`
                                    // duration if windows are fully static) and then yield to node (for gc & timers)
                                    let poll_time = Duration::from_nanos(1_000_000_000 / NODE_POLL_HZ);
                                    event_loop.pump_app_events(Some(poll_time), &mut AppHandler{app: &mut *app, dispatch});
                                    Ok(app.cadence.should_continue() || !app.windows.is_empty())
                                }
                            }
                        })
                    })
                }).join();

                match keep_running{
                    Ok(true) => continue,
                    _ => break
                }
            }

            // resolve the promise
            deferred.settle_with(&channel, move |mut cx| Ok(cx.undefined()) );
        });
    }

    fn dispatch_events(cx:&mut TaskContext, events:Value, window_mgr:Option<&mut WindowManager>) -> NeonResult<()>{
        // window_mgr is only present if it's time to collect updated canvas contents from js
        let is_render = window_mgr.is_some();

        // js callback is passed render flag & json-encoded event queue
        let mut call = match RENDER_CALLBACK.get(){
            None => return Ok(()),
            Some(callback)=> callback.to_inner(cx).call_with(cx),
        };
        call.arg(cx.boolean(is_render))
            .arg(cx.string(events.to_string()));

        match window_mgr{
            None => call.exec(cx)?, // if this is just a UI-event delivery, fire & forget

            Some(window_mgr) => {
                // for a full roundtrip, first pass events to js
                let response = call.apply::<JsValue, _>(cx)?
                    .downcast::<JsArray, _>(cx).or_throw(cx)?
                    .to_vec(cx)?;

                // then unpack the returned window specs & contexts
                let specs_json = response[0].downcast::<JsString, _>(cx).or_throw(cx)?.value(cx);
                let specs:Vec<WindowSpec> = serde_json::from_str(&specs_json)
                    .or_else(|err| cx.throw_error(format!("Malformed response from window event handler: {}", err)) )?;

                let contexts = response[1].downcast::<JsArray, _>(cx).or_throw(cx)?.to_vec(cx)?;
                let pages = contexts.iter().map(|boxed|
                    boxed.downcast::<BoxedContext2D, _>(cx).ok()
                        .map(|ctx| ctx.borrow().get_page())
                );

                // update each window with its new state & content
                zip(specs, pages)
                    .filter_map(|(spec, page)| page.map(|page| (spec, page) ))
                    .for_each(|(spec, page)| window_mgr.update_window(spec, page) );
            }
        };

        Ok(())
    }

}

// ephemeral event handler: borrows the persistent App state and a dispatch closure (which
// holds the neon context for the current tick) only for the duration of a single pump/run call
struct AppHandler<'a, F>{
    app: &'a mut App,
    dispatch: F,
}

impl<F> ApplicationHandler<AppEvent> for AppHandler<'_, F>
    where F:FnMut(Value, Option<&mut WindowManager>) -> NeonResult<()>
{
    fn resumed(&mut self, _event_loop:&ActiveEventLoop){}

    fn window_event(&mut self, event_loop:&ActiveEventLoop, window_id:WindowId, event:WindowEvent){
        let Self{app, dispatch} = self;
        app.windows.find(&window_id, |win| win.sieve.capture(&event) );

        match event {
            WindowEvent::Destroyed | WindowEvent::CloseRequested => {
                app.windows.remove(&window_id);

                // after the last window is closed, either exit (in run_app_on_demand mode)
                // or wait for the window destructor to run (in pump_app_events mode)
                if app.windows.is_empty(){
                    app.vsync = None; // stop ticking once no windows remain
                    match app.mode{
                        LoopMode::Native => event_loop.exit(),
                        LoopMode::Node => app.cadence.loop_again(),
                    }
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
                app.windows.find(&window_id, |win| win.set_fullscreen(false) );
            }

            WindowEvent::Moved(loc) => {
                app.windows.find(&window_id, |win| win.did_move(loc) );
            }

            WindowEvent::Resized(size) => {
                app.windows.find(&window_id, |win| win.did_resize(size) );

                // dispatch the resize to js and repaint with the updated canvas content before
                // returning: during a live-resize the OS runs a modal loop, so waiting for the
                // next frame tick would leave the window contents lagging behind the new size
                dispatch(app.windows.get_ui_changes(), Some(&mut app.windows)).ok();
                app.windows.find(&window_id, |win| win.redraw() );
            }

            #[cfg(target_os = "macos")]
            WindowEvent::Occluded(is_hidden) => {
                app.windows.find(&window_id, |win| win.set_redrawing_suspended(is_hidden) );
            }

            WindowEvent::RedrawRequested => {
                app.windows.find(&window_id, |win| win.redraw() );
            }

            _ => {}
        }
    }

    fn user_event(&mut self, event_loop:&ActiveEventLoop, event:AppEvent){
        let Self{app, dispatch} = self;
        match event{
            AppEvent::Open(spec, page) => {
                app.windows.add(event_loop, spec, page);
                dispatch(app.windows.get_geometry(), Some(&mut app.windows)).ok();

                // only listen for vblank signals when a window is actually open
                if app.vsync.is_none(){
                    let proxy = PROXY.with_borrow(|proxy| proxy.clone());
                    app.vsync = Some(VsyncSource::start(proxy, app.windows.refresh_interval()));
                }
            }
            AppEvent::Close(token) => {
                app.windows.remove_by_token(token);
            }
            AppEvent::FrameRate(fps) => {
                app.cadence.set_frame_rate(fps)
            }
            AppEvent::Tick{ at } => {
                // a tick arrives every vblank and the cadence handles triggering roundtrips & redraws (at the target fps)
                if app.cadence.tick(at){
                    dispatch(app.windows.get_ui_changes(), Some(&mut app.windows)).ok();
                }
            }
            AppEvent::Quit => {
                app.vsync = None;
                event_loop.exit();
            }
        }
    }
}


// how often the event_pump wakes to service node's event loop when no Tick has arrived in the meantime
const NODE_POLL_HZ: u64 = 60;

// The frame heartbeat: a per-platform source that fires a Tick at each display vblank.
struct VsyncSource{
    running: Arc<AtomicBool>,
}

impl VsyncSource{
    fn start(proxy:EventLoopProxy<AppEvent>, interval:Duration) -> Self{
        let running = Arc::new(AtomicBool::new(true));
        let flag = running.clone();
        std::thread::spawn(move ||{
            while flag.load(Ordering::Relaxed){
                spin_sleep::sleep(interval);
                // stop spinning if the event loop has gone away
                if proxy.send_event(AppEvent::Tick{ at: Instant::now() }).is_err(){
                    break;
                }
            }
        });
        Self{ running }
    }
}

impl Drop for VsyncSource{
    fn drop(&mut self){
        self.running.store(false, Ordering::Relaxed);
    }
}

// uses the hardware vblank timing to trigger renders at the js App's target fps, tracking
// fractional frames via `credit` to match the fps on average if it doesn't divide evenly
// into vblank
struct Cadence{
    rate: u64,
    credit: f64,
    last_tick: Instant,
    seeded: bool,
    needs_cleanup: Option<bool>,
}

impl Default for Cadence {
    fn default() -> Self {
        Self{
            rate: 60,
            credit: 0.0,
            last_tick: Instant::now(),
            seeded: false,
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

    // report whether a frame is due (based on target fps) whenever a vblank tick arrives
    fn tick(&mut self, at:Instant) -> bool{
        // the first tick just establishes the time base (and draws immediately)
        if !self.seeded{
            self.seeded = true;
            self.last_tick = at;
            return true;
        }

        // compare the elapsed time since our last redraw to the target fps and see if it's time for another
        let dt = at.saturating_duration_since(self.last_tick).as_secs_f64();
        self.last_tick = at;
        self.credit += self.rate as f64 * dt;

        if self.credit >= 1.0 {
            // preserve fractional remainder so non-perfect divisions will average out
            self.credit = (self.credit - 1.0).min(1.0);
            true
        } else {
            false
        }
    }
}
