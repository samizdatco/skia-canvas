use std::time::Instant;
use winit::event_loop::EventLoopProxy;
use super::event::AppEvent;
use super::window_mgr::WindowManager;

// uses the hardware vblank timing to trigger renders at the js App's target fps, tracking
// fractional frames via `credit` to match the fps on average if it doesn't divide evenly
// into vblank
pub struct Cadence{
    rate: u64,
    credit: f64,
    last_tick: Option<Instant>, // None until the first tick seeds the time base
    needs_cleanup: bool,
    vblank: Option<vblank::Source>,
}

impl Default for Cadence {
    fn default() -> Self {
        Self{
            rate: 60,
            credit: 0.0,
            last_tick: None,
            needs_cleanup: true, // ensure at least one post-Init loop
            vblank: None,
        }
    }
}

impl Cadence{
    // start up the vblank source if it's not already running
    pub fn run(&mut self, proxy:EventLoopProxy<AppEvent>, windows:&WindowManager){
        if self.vblank.is_none(){
            self.vblank = Some(vblank::Source::start(proxy, windows));

            // on Wayland we need to kick off an initial redraw request to start ticking
            if self.is_redraw_driven(){ windows.request_redraw_all(); }
        }
    }

    // stop the source and clear pacing state so restarts don't use an ancient last_tick
    pub fn stop(&mut self){
        self.vblank = None;
        self.last_tick = None;
        self.credit = 0.0;
    }

    pub fn loop_again(&mut self){
        // flag that a clean-up event-loop pass is necessary (e.g., for reflecting window closures)
        self.needs_cleanup = true
    }

    pub fn should_continue(&mut self) -> bool{
        std::mem::take(&mut self.needs_cleanup)
    }

    pub fn set_frame_rate(&mut self, rate:u64){
        self.rate = rate;
    }

    // Wayland's redraw-driven mode has no thread — the RedrawRequested handler runs the cadence
    pub fn is_redraw_driven(&self) -> bool{
        matches!(self.vblank, Some(vblank::Source::RedrawEvent))
    }

    pub fn is_idle(&self) -> bool{
        self.vblank.is_none()
    }

    // report whether a frame is due (based on target fps) whenever a vblank tick arrives
    pub fn tick(&mut self, at:Instant) -> bool{
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


mod vblank {
    use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
    use std::time::{Duration, Instant};
    use winit::event_loop::EventLoopProxy;
    use crate::gui::event::AppEvent;
    use crate::gui::window_mgr::WindowManager;

    // The frame heartbeat: a per-platform source that fires a Tick at each display vblank.
    pub enum Source{
        #[cfg(target_os = "macos")]
        Callback(mac::DisplayLink), // the CoreVideo vblank callback trigger
        Thread(SourceThread), // any blocking-wait / timer source: DwmFlush, drmWaitVBlank, or the plain timer
        RedrawEvent, // Wayland uses winit's RedrawRequested rather than the thread
    }

    impl Source{
        pub fn start(proxy:EventLoopProxy<AppEvent>, window_mgr:&WindowManager) -> Self{
            let interval = window_mgr.refresh_interval();

            // SKIA_CANVAS_VBLANK=off (also 0/false) disables the real per-platform vblank source and
            // drives frames from the plain refresh-rate timer (for debugging misbehaving systems)
            let vblank_disabled = std::env::var("SKIA_CANVAS_VBLANK")
                .map(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "0" | "false" | "off"))
                .unwrap_or(false);

            // try to find a real vblank source or fall back to using an un-anchored timer
            if !vblank_disabled{
                #[cfg(all(unix, not(target_os = "macos")))]
                if std::env::var_os("WAYLAND_DISPLAY").is_some(){
                    return Source::RedrawEvent;
                }

                #[cfg(target_os = "macos")]
                if let Some(link) = window_mgr.primary_display_id()
                    .and_then(|id| mac::start(proxy.clone(), id)){
                    return Source::Callback(link);
                }

                #[cfg(target_os = "windows")]
                if let Some(thread) = windows::start(proxy.clone(), interval){
                    return Source::Thread(thread);
                }

                #[cfg(all(unix, not(target_os = "macos")))]
                if let Some(thread) = linux::start(proxy.clone(), interval){
                    return Source::Thread(thread);
                }
            }

            Source::Thread(timer_thread(proxy, interval))
        }
    }


    // a thread for use with vblank sources that block (DwmFlush, drmWaitVBlank, or a sleep)
    // to signal screen refresh. halts when dropped (e.g., when no windows are animating)
    pub struct SourceThread{
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

    //
    // macOS: drive ticks from the display's actual vblank via CVDisplayLink (CoreVideo)
    //
    #[cfg(target_os = "macos")]
    pub mod mac{
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
        unsafe extern "C" {
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
        unsafe extern "C" { fn mach_timebase_info(info:*mut MachTimebaseInfo) -> i32; }

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
                let proxy = unsafe{ &*(ctx as *const EventLoopProxy<AppEvent>) };
                let at = if output_time.is_null(){
                    // fall back to using `now` only if there's been an error
                    Instant::now()
                }else{
                    // Instants can't be created from raw timestamps, so cache an initial Instant & host_time,
                    // then return that Instant plus the (rate-scaled) delta between the current & cached host_times
                    let host_time = unsafe{ (*(output_time as *const CVTimeStamp)).host_time };
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

    //
    // Windows: block on the DWM compositor's vblank (or fall back to the timer if composition is disabled)
    //
    #[cfg(target_os = "windows")]
    pub mod windows{
        use std::sync::atomic::Ordering;
        use std::time::{Duration, Instant};
        use winit::event_loop::EventLoopProxy;
        use crate::gui::event::AppEvent;
        use super::SourceThread;

        #[link(name = "dwmapi")]
        unsafe extern "system"{
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

    //
    // Linux (X11): block on the GPU's vblank via DRM (or fall back to the timer if running headless, lacking device permission, etc.)
    //
    #[cfg(all(unix, not(target_os = "macos")))]
    pub mod linux{
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

}
