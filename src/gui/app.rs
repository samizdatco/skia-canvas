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
    cadence::Cadence,
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

fn get_proxy() -> EventLoopProxy<AppEvent>{
    PROXY.with_borrow_mut(|proxy| proxy.clone())
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
                let keep_running = channel.send(move |cx| {
                    // wait for events to arrive (unless interrupted by a vblank tick or 'idle wake'
                    // duration if windows are fully static) and then yield to node (for gc & timers).
                    APP.with_borrow_mut(|app| {
                        EVENT_LOOP.with_borrow_mut(|event_loop|{
                            event_loop.pump_app_events(
                              Some(Duration::from_millis(NODE_IDLE_WAKE_MS)),
                              &mut AppHandler{app: &mut *app, cx}
                            );
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

    // run the per-frame javascript call, passing a payload of queued changes and receiving back
    // canvas contents (and possibly programmatic window state changes). also updates vblank status
    // based on whether any of the windows have `draw` or `frame` listeners
    fn roundtrip(&mut self, cx:&mut TaskContext, with_payload:With){
        // collect the outbound payload (draining each window's sieve as a side effect)
        let changes = match with_payload{
            With::Events => self.windows.get_ui_changes(),
            With::Geometry => self.windows.get_geometry(),
        };
        let windows = &mut self.windows;

        // track whether any windows have ongoing animations in need of vblank ticks
        let mut is_animating = true; // to be updated with response from js

        // enclose js<->rust bridging in a nested scope to release the js handles immediately rather
        // than letting them accumulate in the pump_events scope
        cx.execute_scoped(|mut cx| -> NeonResult<()> {
            let cx = &mut cx;

            // pass the json-encoded event queue to the js callback and collect its response array
            let Some(callback) = RENDER_CALLBACK.get() else { return Ok(()) };
            let mut call = callback.to_inner(cx).call_with(cx);
            call.arg(cx.string(changes.to_string()));
            let response = call.apply::<JsValue, _>(cx)?
                .downcast::<JsArray, _>(cx).or_throw(cx)?
                .to_vec(cx)?;

            // unpack the returned window specs, contexts, and is_animating flag
            let specs_json = response[0].downcast::<JsString, _>(cx).or_throw(cx)?.value(cx);
            let specs:Vec<WindowSpec> = serde_json::from_str(&specs_json)
                .or_else(|err| cx.throw_error(format!("Malformed response from window event handler: {}", err)) )?;

            // extract page snapshots from the contexts
            let contexts = response[1].downcast::<JsArray, _>(cx).or_throw(cx)?.to_vec(cx)?;
            let pages = contexts.iter().map(|boxed|
                boxed.downcast::<BoxedContext2D, _>(cx).ok()
                    .map(|ctx| ctx.borrow().get_page())
            );

            // update windows with the unpacked specs & pages
            zip(specs, pages)
                .filter_map(|(spec, page)| page.map(|page| (spec, page) ))
                .for_each(|(spec, page)| windows.update_window(spec, page) );

            // note whether any window still has a frame/draw listener, so vblank can be paused when idle
            is_animating = response.get(2)
                .and_then(|val| val.downcast::<JsBoolean, _>(cx).ok())
                .map(|flag| flag.value(cx))
                .unwrap_or(true);

            Ok(())
        }).ok();

        // check the updated animation status and only have the vblank source run if needed
        if is_animating{
            self.cadence.run(get_proxy(), &self.windows);
        }else if !self.cadence.is_idle(){
            self.cadence.stop();
        }
    }
}

// identify the app state that should be passed to the js side in a roundtrip() call
enum With { Events, Geometry }

// ephemeral event handler: borrows the persistent App state and holds the neon context for the
// current tick, only for the duration of a single pump/run call. carrying `cx` here is what lets
// winit event handlers make synchronous js roundtrips mid-pump (see `roundtrip`).
struct AppHandler<'a, 'cx>{
    app: &'a mut App,
    cx: TaskContext<'cx>,
}


impl ApplicationHandler<AppEvent> for AppHandler<'_, '_>{
    fn resumed(&mut self, _event_loop:&ActiveEventLoop){}

    fn window_event(&mut self, _event_loop:&ActiveEventLoop, window_id:WindowId, event:WindowEvent){
        let Self{app, cx} = self;
        app.windows.find(&window_id, |win|{
            let dpr = win.handle.scale_factor();
            win.sieve.capture(&event, dpr);
        });

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
                app.windows.find(&window_id, |win|{
                    win.did_move(loc);
                    win.update_monitor();
                });
            }

            WindowEvent::ScaleFactorChanged{inner_size_writer: _, ..} => {
                // ignore inner_size_writer and leave logical size unchanged while phys rescales.
                // a Resized event won't always accompany this, so call update_fit just in case
                app.windows.find(&window_id, |win|{
                    win.update_fit();
                    win.update_monitor();
                });
            }

            WindowEvent::Resized(size) => {
                app.windows.find(&window_id, |win|{
                    win.did_resize(size);
                    win.update_monitor(); // fullscreen moves can resize without a `Moved`
                });

                // dispatch the resize to js and repaint with the updated canvas content before
                // returning: during a live-resize the OS runs a modal loop, so waiting for the
                // next frame tick would leave the window contents lagging behind the new size
                app.roundtrip(cx, With::Events);
                app.windows.find(&window_id, |win| win.redraw() );
            }

            #[cfg(target_os = "macos")]
            WindowEvent::Occluded(is_hidden) => {
                app.windows.find(&window_id, |win| win.set_redrawing_suspended(is_hidden) );
            }

            WindowEvent::RedrawRequested => {
                if app.cadence.is_redraw_driven(){
                    // on wayland, this event *is* the vblank, so it's time to run the js-roundtrip
                    // (all other platforms trigger the roundtrip through AppEvent::Tick)
                    if app.cadence.tick(Instant::now()){
                        app.roundtrip(cx, With::Events); // may drop vblank need if app goes idle
                    }
                    app.windows.find(&window_id, |win|{
                        win.redraw(); // update the window contents immediately

                        // only request the next vblank if the app is still animating post-roundtrip
                        if app.cadence.is_redraw_driven(){
                            win.handle.request_redraw();
                        }
                    });
                }else{
                    // on all other platforms just update the window
                    app.windows.find(&window_id, |win|{ win.redraw(); });
                }
            }

            _ => {}
        }

        // the global vblank is driven by the first window's monitor. if that 'main' monitor goes
        // away (or the window moves to another monitor), drop the vblank source and let it be
        // recreated (on the newly-main monitor) by the roundtrip below
        if app.windows.main_monitor_changed() && !app.cadence.is_idle(){
            app.cadence.stop();
        }

        // when idle (no vblank source) the loop is event-driven: flush queued UI events
        //  to js and repaint right away, since no Tick event will arrive to do it
        if app.cadence.is_idle(){
            app.roundtrip(cx, With::Events);
        }
    }

    fn user_event(&mut self, event_loop:&ActiveEventLoop, event:AppEvent){
        let Self{app, cx} = self;
        match event{
            AppEvent::Open(spec, page) => {
                app.windows.add(event_loop, spec, page);
                app.roundtrip(cx, With::Geometry);
            }
            AppEvent::Close(token) => {
                app.windows.remove_by_token(token);

                // closing the first window can shift which display the vblank source should pace against
                if app.windows.main_monitor_changed() && !app.cadence.is_idle(){
                    app.cadence.stop();
                    app.roundtrip(cx, With::Events); // rebuilds the source if still animating
                }
            }
            AppEvent::FrameRate(fps) => {
                app.cadence.set_frame_rate(fps)
            }
            AppEvent::Tick{ at } => {
                // a tick arrives every vblank and the cadence handles triggering roundtrips & redraws (at the target fps)
                if app.cadence.tick(at){
                    app.roundtrip(cx, With::Events); // pause the source if the app just went idle
                }
            }
            AppEvent::Quit => {
                app.cadence.stop();
                event_loop.exit();
            }
        }
    }
}
