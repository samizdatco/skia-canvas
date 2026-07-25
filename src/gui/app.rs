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
    event::{ElementState, KeyEvent, WindowEvent},
    event_loop::{EventLoop, EventLoopProxy, ActiveEventLoop},
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

// how often the event_pump wakes to service node's event loop when no Tick has arrived in the meantime
const NODE_IDLE_WAKE_MS: u64 = 100;

pub struct App{
    windows: WindowManager,
    cadence: Cadence,
}

impl Default for App{
    fn default() -> Self {
        Self{
            windows: WindowManager::default(),
            cadence: Cadence::default(),
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

                    // wait for events to arrive (unless interrupted by a vsync tick or `backstop`
                    // duration if windows are fully static) and then yield to node (for gc & timers)
                    APP.with_borrow_mut(|app| {
                        EVENT_LOOP.with_borrow_mut(|event_loop|{
                            let backstop = Duration::from_millis(NODE_IDLE_WAKE_MS);
                            event_loop.pump_app_events(Some(backstop), &mut AppHandler{app: &mut *app, dispatch});
                            Ok(app.cadence.should_continue() || !app.windows.is_empty())
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

    // creates a vsync source if animating and one doesn't exist, drops the source when idle
    // (called after every js roundtrip, when the `animating` flag is updated)
    fn ensure_vsync(&mut self){
        let want = self.windows.is_animating() && !self.windows.is_empty();
        if want && self.cadence.is_idle(){
            let proxy = PROXY.with_borrow(|proxy| proxy.clone());
            self.cadence.run(proxy, &self.windows);
            if self.cadence.is_redraw_driven(){ self.windows.request_redraw_all(); }
        }else if !want && !self.cadence.is_idle(){
            self.cadence.stop();
        }
    }

    fn dispatch_events(cx:&mut TaskContext, events:Value, window_mgr:Option<&mut WindowManager>) -> NeonResult<()>{
        // run the per-frame javascript-roundtrip in its own scope to release the js<->rust payloads
        // immediately rather than letting them accumulate in the pump_events scope
        cx.execute_scoped(|mut cx| -> NeonResult<()> {
            let cx = &mut cx;

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

                    // note whether any window still has a frame/draw listener, so vsync can be paused when idle
                    let animating = response.get(2)
                        .and_then(|val| val.downcast::<JsBoolean, _>(cx).ok())
                        .map(|flag| flag.value(cx))
                        .unwrap_or(true);
                    window_mgr.set_animating(animating);
                }
            };

            Ok(())
        })
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

    fn window_event(&mut self, _event_loop:&ActiveEventLoop, window_id:WindowId, event:WindowEvent){
        let Self{app, dispatch} = self;
        app.windows.find(&window_id, |win| win.sieve.capture(&event) );

        match event {
            WindowEvent::Destroyed | WindowEvent::CloseRequested => {
                app.windows.remove(&window_id);

                // after the last window is closed...
                if app.windows.is_empty(){
                    app.cadence.stop(); // stop ticking once no windows remain
                    app.cadence.loop_again(); // run one more cycle to let its destructor run
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
                app.ensure_vsync();
                app.windows.find(&window_id, |win| win.redraw() );
            }

            #[cfg(target_os = "macos")]
            WindowEvent::Occluded(is_hidden) => {
                app.windows.find(&window_id, |win| win.set_redrawing_suspended(is_hidden) );
            }

            WindowEvent::RedrawRequested => {
                // on wayland, this event *is* the vsync, so it's time to run the js-roundtrip
                // (all other platforms trigger the roundtrip through AppEvent::Tick)
                if app.cadence.is_redraw_driven() && app.cadence.tick(Instant::now()){
                    dispatch(app.windows.get_ui_changes(), Some(&mut app.windows)).ok();
                    app.ensure_vsync(); // may drop the source if the app just went idle
                }
                app.windows.find(&window_id, |win|{
                    win.redraw(); // all platforms update the window

                    // wayland also needs to request the next vsync callback
                    if app.cadence.is_redraw_driven(){ win.handle.request_redraw(); }
                });
            }

            _ => {}
        }

        // when idle (no vsync source) the loop is event-driven: flush queued UI events to js and
        // repaint right away, since no frame tick will arrive to do it. a handler may start
        // animating, so reconcile the source afterward.
        if app.cadence.is_idle(){
            dispatch(app.windows.get_ui_changes(), Some(&mut app.windows)).ok();
            app.ensure_vsync();
        }
    }

    fn user_event(&mut self, event_loop:&ActiveEventLoop, event:AppEvent){
        let Self{app, dispatch} = self;
        match event{
            AppEvent::Open(spec, page) => {
                app.windows.add(event_loop, spec, page);
                dispatch(app.windows.get_geometry(), Some(&mut app.windows)).ok();
                app.ensure_vsync(); // only listen for vblank signals when ≥1 window is animating
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
                    app.ensure_vsync(); // pause the source if the app just went idle
                }
            }
            AppEvent::Quit => {
                app.cadence.stop();
                event_loop.exit();
            }
        }
    }
}


// The frame heartbeat: a per-platform source that fires a Tick at each display vblank.
enum VsyncSource{
    // any blocking-wait / timer source: DwmFlush, drmWaitVBlank, or the plain timer
    Thread(SourceThread),
    #[cfg(target_os = "macos")]
    DisplayLink(corevideo_vsync::DisplayLink),
    RedrawDriven, // Wayland uses winit's RedrawRequested rather than the thread
}

impl VsyncSource{
    fn start(proxy:EventLoopProxy<AppEvent>, windows:&WindowManager) -> Self{
        let interval = windows.refresh_interval();

        // try to find a real vblank source or fall back to using an un-anchored timer
        if !vsync_disabled{
            #[cfg(all(unix, not(target_os = "macos")))]
            if std::env::var_os("WAYLAND_DISPLAY").is_some(){
                return VsyncSource::RedrawDriven;
            }

            #[cfg(target_os = "macos")]
            if let Some(link) = windows.primary_display_id()
                .and_then(|id| corevideo_vsync::start(proxy.clone(), id)){
                return VsyncSource::DisplayLink(link);
            }

            #[cfg(target_os = "windows")]
            if let Some(thread) = dwm_vsync::start(proxy.clone(), interval){
                return VsyncSource::Thread(thread);
            }

            #[cfg(all(unix, not(target_os = "macos")))]
            if let Some(thread) = drm_vsync::start(proxy.clone(), interval){
                return VsyncSource::Thread(thread);
            }
        }

        VsyncSource::Thread(timer_thread(proxy, interval))
    }
}


// a thread for use with vblank sources that block (DwmFlush, drmWaitVBlank, or a sleep)
// to signal screen refresh. halts when dropped (e.g., when no windows are animating)
struct SourceThread{
    running: Arc<AtomicBool>,
}

impl SourceThread{
    fn spawn<F>(body:F) -> Self where F:FnOnce(Arc<AtomicBool>) + Send + 'static{
        let running = Arc::new(AtomicBool::new(true));
        let flag = running.clone();
        std::thread::spawn(move || body(flag));
        Self{ running }
    }
}

impl Drop for SourceThread{
    fn drop(&mut self){
        self.running.store(false, Ordering::Relaxed);
    }
}

// cross-platform fallback: sleep at the refresh interval, then post a Tick
fn timer_thread(proxy:EventLoopProxy<AppEvent>, interval:Duration) -> SourceThread{
    SourceThread::spawn(move |flag|{
        while flag.load(Ordering::Relaxed){
            spin_sleep::sleep(interval);
            // stop spinning if the event loop has gone away
            if proxy.send_event(AppEvent::Tick{ at: Instant::now() }).is_err(){
                break;
            }
        }
    })
}

// Windows: block on the DWM compositor's vblank (or fall back to the timer if composition is disabled)
#[cfg(target_os = "windows")]
mod dwm_vsync{
    use std::sync::atomic::Ordering;
    use std::time::{Duration, Instant};
    use winit::event_loop::EventLoopProxy;
    use crate::gui::event::AppEvent;
    use super::SourceThread;

    #[link(name = "dwmapi")]
    extern "system"{
        fn DwmFlush() -> i32;
        fn DwmIsCompositionEnabled(enabled:*mut i32) -> i32;
    }

    pub fn start(proxy:EventLoopProxy<AppEvent>, fallback:Duration) -> Option<SourceThread>{
        // DwmFlush only tracks vblank while the compositor is running (always on for Win8+)
        let mut enabled:i32 = 0;
        if unsafe{ DwmIsCompositionEnabled(&mut enabled) } != 0 || enabled == 0{
            return None;
        }
        Some(SourceThread::spawn(move |flag|{
            while flag.load(Ordering::Relaxed){
                // block until the next DWM vblank
                if unsafe{ DwmFlush() } != 0{
                    // if composition drops out mid-run sleep at the refresh interval so ticks don't stop
                    spin_sleep::sleep(fallback);
                }
                if proxy.send_event(AppEvent::Tick{ at: Instant::now() }).is_err(){ break; }
            }
        }))
    }
}

// X11/Linux: block on the GPU's vblank via DRM (or fall back to the timer if running headless, lacking device permission, etc.)
#[cfg(all(unix, not(target_os = "macos")))]
mod drm_vsync{
    use std::fs::{File, OpenOptions};
    use std::os::unix::io::{AsFd, BorrowedFd};
    use std::sync::atomic::Ordering;
    use std::time::{Duration, Instant};
    use drm::{Device as DrmDevice, VblankWaitTarget, VblankWaitFlags};
    use winit::event_loop::EventLoopProxy;
    use crate::gui::event::AppEvent;
    use super::SourceThread;

    // minimal DRM device wrapper (per the drm crate's documented pattern)
    struct Device(File);
    impl AsFd for Device{
        fn as_fd(&self) -> BorrowedFd<'_>{ self.0.as_fd() }
    }
    impl DrmDevice for Device{}

    pub fn start(proxy:EventLoopProxy<AppEvent>, fallback:Duration) -> Option<SourceThread>{
        // Wayland has its own (redraw-driven) source; DRM is for the X11 / no-compositor path
        if std::env::var_os("WAYLAND_DISPLAY").is_some(){ return None; }

        // open the first primary node whose vblank works (i.e., disregard headless GPUs)
        let device = std::fs::read_dir("/dev/dri").ok()?
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_name().as_encoded_bytes().starts_with(b"card"))
            .find_map(|entry|{
                let file = OpenOptions::new().read(true).write(true).open(entry.path()).ok()?;
                let device = Device(file);
                device.wait_vblank(VblankWaitTarget::Relative(1), VblankWaitFlags::empty(), 0, 0).ok()?;
                Some(device)
            })?;

        Some(SourceThread::spawn(move |flag|{
            while flag.load(Ordering::Relaxed){
                // block until the next vblank
                if device.wait_vblank(VblankWaitTarget::Relative(1), VblankWaitFlags::empty(), 0, 0).is_err(){
                    // on error, sleep at the refresh interval so ticks don't stop
                    spin_sleep::sleep(fallback);
                }
                if proxy.send_event(AppEvent::Tick{ at: Instant::now() }).is_err(){ break; }
            }
        }))
    }
}

// macOS: drive ticks from the display's actual vblank via CVDisplayLink (CoreVideo)
#[cfg(target_os = "macos")]
mod corevideo_vsync{
    use std::ffi::c_void;
    use std::time::{Instant, Duration};
    use std::sync::OnceLock;
    use winit::event_loop::EventLoopProxy;
    use crate::gui::event::AppEvent;

    type CVDisplayLinkRef = *mut c_void; // opaque handle
    type CVReturn = i32;
    type CGDirectDisplayID = u32;
    type CVOptionFlags = u64;
    const CV_RETURN_SUCCESS: CVReturn = 0;

    type OutputCallback = unsafe extern "C" fn(
        CVDisplayLinkRef, *const c_void, *const c_void, CVOptionFlags, *mut CVOptionFlags, *mut c_void,
    ) -> CVReturn;

    #[link(name = "CoreVideo", kind = "framework")]
    extern "C" {
        fn CVDisplayLinkCreateWithCGDisplay(display:CGDirectDisplayID, out:*mut CVDisplayLinkRef) -> CVReturn;
        fn CVDisplayLinkSetOutputCallback(link:CVDisplayLinkRef, cb:OutputCallback, ctx:*mut c_void) -> CVReturn;
        fn CVDisplayLinkStart(link:CVDisplayLinkRef) -> CVReturn;
        fn CVDisplayLinkStop(link:CVDisplayLinkRef) -> CVReturn;
        fn CVDisplayLinkRelease(link:CVDisplayLinkRef);
    }

    #[repr(C)]
    struct CVTimeStamp{ version:u32, video_time_scale:i32, video_time:i64, host_time:u64 }

    #[repr(C)]
    struct MachTimebaseInfo{ numer:u32, denom:u32 }
    extern "C" { fn mach_timebase_info(info:*mut MachTimebaseInfo) -> i32; }

    pub struct DisplayLink{
        link: CVDisplayLinkRef,
        _proxy: Box<EventLoopProxy<AppEvent>>, // boxed so on_vblank gets a stable address
    }

    impl DisplayLink{
        // fires on CVDisplayLink's own thread at each vblank
        unsafe extern "C" fn on_vblank(
            _link:CVDisplayLinkRef,
            _now:*const c_void,
            output_time:*const c_void,
            _flags_in:CVOptionFlags,
            _flags_out:*mut CVOptionFlags,
            ctx:*mut c_void, // points to the boxed proxy
        ) -> CVReturn {
            let proxy = &*(ctx as *const EventLoopProxy<AppEvent>);
            let at = if output_time.is_null(){
                // fall back to using `now` only if there's been an error
                Instant::now()
            }else{
                // Instants can't be created from raw timestamps, so cache an initial Instant & host_time,
                // then return that Instant plus the (rate-scaled) delta between the current & cached host_times
                let host_time = (*(output_time as *const CVTimeStamp)).host_time;
                static BASE:OnceLock<(Instant, u64, u32, u32)> = OnceLock::new();
                let (base_instant, base_host, numer, denom) = *BASE.get_or_init(||{
                    let mut tb = MachTimebaseInfo{ numer:0, denom:0 };
                    unsafe{ mach_timebase_info(&mut tb); } // record the *rate* that host_time passes at
                    (Instant::now(), host_time, tb.numer.max(1), tb.denom.max(1))
                });
                let delta_ns = host_time.saturating_sub(base_host) as u128 * numer as u128 / denom as u128;
                base_instant + Duration::from_nanos(delta_ns as u64)
            };
            let _ = proxy.send_event(AppEvent::Tick{ at });
            CV_RETURN_SUCCESS
        }
    }

    impl Drop for DisplayLink{
        fn drop(&mut self){
            unsafe{
                CVDisplayLinkStop(self.link);
                CVDisplayLinkRelease(self.link);
            }
        }
    }

    pub fn start(proxy:EventLoopProxy<AppEvent>, display_id:CGDirectDisplayID) -> Option<DisplayLink>{
        unsafe{
            // create the link
            let mut link:CVDisplayLinkRef = std::ptr::null_mut();
            if CVDisplayLinkCreateWithCGDisplay(display_id, &mut link) != CV_RETURN_SUCCESS || link.is_null(){
                return None;
            }

            // connect the link to our on_vblank callback
            let boxed = Box::new(proxy);
            let ctx = (&*boxed as *const EventLoopProxy<AppEvent>) as *mut c_void;
            if CVDisplayLinkSetOutputCallback(link, DisplayLink::on_vblank, ctx) != CV_RETURN_SUCCESS
                || CVDisplayLinkStart(link) != CV_RETURN_SUCCESS
            {
                CVDisplayLinkRelease(link);
                return None;
            }

            Some(DisplayLink{ link, _proxy: boxed })
        }
    }

}

// uses the hardware vblank timing to trigger renders at the js App's target fps, tracking
// fractional frames via `credit` to match the fps on average if it doesn't divide evenly
// into vblank
struct Cadence{
    rate: u64,
    credit: f64,
    last_tick: Option<Instant>, // None until the first tick seeds the time base
    needs_cleanup: bool,
    vsync: Option<VsyncSource>,
}

impl Default for Cadence {
    fn default() -> Self {
        Self{
            rate: 60,
            credit: 0.0,
            last_tick: None,
            needs_cleanup: true, // ensure at least one post-Init loop
            vsync: None,
        }
    }
}

impl Cadence{
    // start up the vsync source if it's not already running
    fn run(&mut self, proxy:EventLoopProxy<AppEvent>, windows:&WindowManager){
        if self.vsync.is_none(){
            self.vsync = Some(VsyncSource::start(proxy, windows));
        }
    }

    // stop the source and clear pacing state so restarts don't use an ancient last_tick
    fn stop(&mut self){
        self.vsync = None;
        self.last_tick = None;
        self.credit = 0.0;
    }

    fn loop_again(&mut self){
        // flag that a clean-up event-loop pass is necessary (e.g., for reflecting window closures)
        self.needs_cleanup = true
    }

    fn should_continue(&mut self) -> bool{
        std::mem::take(&mut self.needs_cleanup)
    }

    fn set_frame_rate(&mut self, rate:u64){
        self.rate = rate;
    }

    // Wayland's redraw-driven mode has no thread — the RedrawRequested handler runs the cadence
    fn is_redraw_driven(&self) -> bool{
        matches!(self.vsync, Some(VsyncSource::RedrawDriven))
    }

    fn is_idle(&self) -> bool{
        self.vsync.is_none()
    }

    // report whether a frame is due (based on target fps) whenever a vblank tick arrives
    fn tick(&mut self, at:Instant) -> bool{
        // the first tick just establishes the time base (and draws immediately)
        let Some(prev) = self.last_tick else {
            self.last_tick = Some(at);
            return true;
        };

        // compare the elapsed time since our last redraw to the target fps and see if it's time for another
        let dt = at.saturating_duration_since(prev).as_secs_f64();
        self.last_tick = Some(at);
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
