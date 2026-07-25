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

                    // wait for events to arrive (unless interrupted by a vblank tick or `backstop`
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

    // creates a vblank source if animating and one doesn't exist, drops the source when idle
    // (called after every js roundtrip, when the `animating` flag is updated)
    fn ensure_vblank(&mut self){
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

                    // note whether any window still has a frame/draw listener, so vblank can be paused when idle
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
                app.ensure_vblank();
                app.windows.find(&window_id, |win| win.redraw() );
            }

            #[cfg(target_os = "macos")]
            WindowEvent::Occluded(is_hidden) => {
                app.windows.find(&window_id, |win| win.set_redrawing_suspended(is_hidden) );
            }

            WindowEvent::RedrawRequested => {
                // on wayland, this event *is* the vblank, so it's time to run the js-roundtrip
                // (all other platforms trigger the roundtrip through AppEvent::Tick)
                if app.cadence.is_redraw_driven() && app.cadence.tick(Instant::now()){
                    dispatch(app.windows.get_ui_changes(), Some(&mut app.windows)).ok();
                    app.ensure_vblank(); // may drop the source if the app just went idle
                }
                app.windows.find(&window_id, |win|{
                    win.redraw(); // all platforms update the window

                    // wayland also needs to request the next vblank callback
                    if app.cadence.is_redraw_driven(){ win.handle.request_redraw(); }
                });
            }

            _ => {}
        }

        // when idle (no vblank source) the loop is event-driven: flush queued UI events to js and
        // repaint right away, since no frame tick will arrive to do it. a handler may start
        // animating, so reconcile the source afterward.
        if app.cadence.is_idle(){
            dispatch(app.windows.get_ui_changes(), Some(&mut app.windows)).ok();
            app.ensure_vblank();
        }
    }

    fn user_event(&mut self, event_loop:&ActiveEventLoop, event:AppEvent){
        let Self{app, dispatch} = self;
        match event{
            AppEvent::Open(spec, page) => {
                app.windows.add(event_loop, spec, page);
                dispatch(app.windows.get_geometry(), Some(&mut app.windows)).ok();
                app.ensure_vblank(); // only listen for vblank signals when ≥1 window is animating
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
                    app.ensure_vblank(); // pause the source if the app just went idle
                }
            }
            AppEvent::Quit => {
                app.cadence.stop();
                event_loop.exit();
            }
        }
    }
}
