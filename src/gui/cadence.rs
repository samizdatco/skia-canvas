use std::time::{Duration, Instant};
use winit::event_loop::EventLoopProxy;
use super::event::AppEvent;
use super::window_mgr::WindowManager;

// uses hardware vblank timing to trigger renders at the js App's target fps, tracking
// fractional refresh intervals via `credit` to match the fps on average if it doesn't divide evenly
// into vblank

pub struct Cadence{
    rate: u64,
    credit: f64,
    needs_cleanup: bool,
    clock: Clock,
    source: Option<vblank::Source>,
}

impl Default for Cadence {
    fn default() -> Self {
        Self{
            rate: 60,
            credit: 0.0,
            clock: Clock::new(Duration::from_secs_f64(1.0 / 60.0)), // 60Hz until the first source starts
            needs_cleanup: true, // ensure at least one post-Init loop
            source: None,
        }
    }
}

impl Cadence{
    // start up the vblank source if it's not already running
    pub fn run(&mut self, proxy:EventLoopProxy<AppEvent>, windows:&WindowManager){
        if self.source.is_none(){
            self.clock = Clock::new(windows.refresh_interval()); // pace against the current display's rate
            self.source = Some(vblank::Source::start(proxy, windows));

            // on Wayland we need to kick off an initial redraw request to start ticking
            if self.is_redraw_driven(){ windows.request_redraw_all(); }
        }
    }

    // stop the source and clear its credit (run() re-seeds the clock when it restarts)
    pub fn stop(&mut self){
        self.source = None;
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
        matches!(self.source, Some(vblank::Source::RedrawEvent))
    }

    pub fn is_idle(&self) -> bool{
        self.source.is_none()
    }

    // report whether a frame is due (based on target fps) whenever a vblank tick arrives, adopting the
    // display's current refresh interval if the source reported one
    pub fn tick(&mut self, at:Instant, period:Option<Duration>) -> bool{
        if let Some(period) = period{ self.clock.set_period(period) }

        // vblanks keep arriving even if the js roundtrip is too slow to support them, so only add new
        // Tick events when the vblank's timestamp isn't from multiple vblank-intervals in our past
        if Instant::now().saturating_duration_since(at) > self.clock.period.mul_f64(1.5) {
            return false; // leave the clock/credit untouched so the next fresh tick paces from here
        }

        // credit the target fps for the refresh intervals since the last tick (the first tick just
        // establishes the time base and draws immediately)
        let Some(vblanks) = self.clock.advance(at) else { return true };
        self.credit += self.rate as f64 * self.clock.period.as_secs_f64() * vblanks;

        // allow half a vblank of slack so frames land on the nearest vblank rather than the next one
        // (otherwise timing jitter makes evenly-divisible rates like 30fps@60Hz flicker between 1v/2v/3v)
        let slack = 0.5 * (self.rate as f64 * self.clock.period.as_secs_f64()).min(1.0);
        if self.credit >= 1.0 - slack {
            // preserve fractional remainder so non-perfect divisions will average out
            self.credit = (self.credit - 1.0).min(1.0);
            true
        } else {
            false
        }
    }
}


// counts the refresh intervals that pass between vblank ticks
struct Clock{
    period: Duration, // the display's nominal refresh interval
    last: Option<Instant>, // when the previous tick arrived
    drift: f64, // smoothed distance of ticks from whole intervals (~0 unless the period is stale)
}

const DRIFT_LIMIT: f64 = 0.05; // average fraction of a vblank that ticks may stray from whole intervals
const DRIFT_STEP: f64 = 0.02; // the most any one tick adds to drift (so a single late tick can't pass the limit)

impl Clock{
    fn new(period:Duration) -> Self{
        let mut clock = Self{ period:Duration::ZERO, last:None, drift:0.0 };
        clock.set_period(period);
        clock
    }

    // update the refresh interval (rounded to whole Hz)
    fn set_period(&mut self, period:Duration){
        let period = Duration::from_secs_f64(1.0 / (1.0 / period.as_secs_f64()).round().max(1.0));
        if period != self.period{
            self.period = period;
            self.drift = 0.0;
        }
    }

    // count the intervals since the last tick (None for the first one)
    fn advance(&mut self, at:Instant) -> Option<f64>{
        let last = self.last.replace(at)?;
        let vblanks = at.saturating_duration_since(last).as_secs_f64() / self.period.as_secs_f64();
        let offset = (vblanks - vblanks.round()).abs();
        self.drift = 0.9 * self.drift + (0.1 * offset).min(DRIFT_STEP);
        Some(if self.drift < DRIFT_LIMIT { vblanks.round() } else { vblanks })
    }
}


mod vblank {
    use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
    use std::time::{Duration, Instant};
    use winit::event_loop::EventLoopProxy;
    use crate::gui::event::AppEvent;
    use crate::gui::window_mgr::WindowManager;

    // The frame heartbeat: a per-platform source that fires a Tick at each display vblank
    #[allow(dead_code)]
    pub enum Source{
        #[cfg(target_os = "macos")]
        Callback(mac::DisplayLink), // the CoreVideo vblank callback trigger
        Thread(SourceThread), // any blocking-wait / timer source: DwmFlush, drmWaitVBlank, or the plain timer
        RedrawEvent, // Wayland uses winit's RedrawRequested rather than the thread
    }

    impl Source{
        pub fn start(proxy:EventLoopProxy<AppEvent>, window_mgr:&WindowManager) -> Self{
            let interval = window_mgr.refresh_interval();

            // try to find a real vblank source or fall back to using an un-anchored timer
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

            Source::Thread(timer_thread(proxy, interval))
        }
    }


    // a thread for use with vblank sources that block (DwmFlush, drmWaitVBlank, or a sleep)
    // to signal screen refresh. halts when dropped (e.g., when no windows are animating)
    pub struct SourceThread{
        running: Arc<AtomicBool>,
    }

    impl SourceThread{
        // wait for the next vblank then post a Tick event (including the source's period)
        fn spawn(proxy:EventLoopProxy<AppEvent>, mut wait:impl FnMut() -> Option<Duration> + Send + 'static) -> Self{
            let running = Arc::new(AtomicBool::new(true));
            let flag = running.clone();
            std::thread::spawn(move ||{
                while flag.load(Ordering::Relaxed){
                    let period = wait();
                    if proxy.send_event(AppEvent::Tick{ at: Instant::now(), period }).is_err(){
                        break // stop if the event loop has gone away
                    }
                }
            });
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
        SourceThread::spawn(proxy, move ||{ spin_sleep::sleep(interval); None })
    }

    //
    // macOS: drive ticks from the display's actual vblank via CVDisplayLink (CoreVideo)
    //
    #[cfg(target_os = "macos")]
    pub mod mac{
        #![allow(deprecated)]

        use std::ffi::c_void;
        use std::ptr::NonNull;
        use std::time::{Instant, Duration};
        use std::sync::OnceLock;
        use objc2_core_foundation::CFRetained;
        use objc2_core_video::{kCVReturnSuccess, CVDisplayLink, CVOptionFlags, CVReturn, CVTimeStamp};
        use winit::event_loop::EventLoopProxy;
        use crate::gui::event::AppEvent;

        const VIDEO_REFRESH_PERIOD_VALID:u64 = 1 << 3; // kCVTimeStampVideoRefreshPeriodValid

        #[repr(C)]
        struct MachTimebaseInfo{ numer:u32, denom:u32 }
        unsafe extern "C" { fn mach_timebase_info(info:*mut MachTimebaseInfo) -> i32; }

        pub struct DisplayLink{
            link: CFRetained<CVDisplayLink>,
            _proxy: Box<EventLoopProxy<AppEvent>>, // boxed so on_vblank gets a stable address
        }

        impl DisplayLink{
            // fires on CVDisplayLink's own thread at each vblank
            unsafe extern "C-unwind" fn on_vblank(
                _link:NonNull<CVDisplayLink>,
                now:NonNull<CVTimeStamp>,           // the vblank that just fired
                _output_time:NonNull<CVTimeStamp>,  // predicted scan-out (unused)
                _flags_in:CVOptionFlags,
                _flags_out:NonNull<CVOptionFlags>,
                ctx:*mut c_void, // points to the boxed proxy
            ) -> CVReturn {
                let proxy = unsafe{ &*(ctx as *const EventLoopProxy<AppEvent>) };
                let now = unsafe{ now.as_ref() };

                // Instants can't be created from raw timestamps, so cache an initial Instant & hostTime,
                // then return that Instant plus the (rate-scaled) delta between the current & cached hostTimes
                static BASE:OnceLock<(Instant, u64, u32, u32)> = OnceLock::new();
                let (base_instant, base_host, numer, denom) = *BASE.get_or_init(||{
                    let mut tb = MachTimebaseInfo{ numer:0, denom:0 };
                    unsafe{ mach_timebase_info(&mut tb); } // record the *rate* that hostTime passes at
                    (Instant::now(), now.hostTime, tb.numer.max(1), tb.denom.max(1))
                });
                let delta_ns = now.hostTime.saturating_sub(base_host) as u128 * numer as u128 / denom as u128;
                let at = base_instant + Duration::from_nanos(delta_ns as u64);

                // report the current display mode's own interval so the count follows mode changes (e.g., 60↔120Hz)
                let valid = now.flags & VIDEO_REFRESH_PERIOD_VALID != 0 && now.videoRefreshPeriod > 0 && now.videoTimeScale > 0;
                let period = valid.then(|| Duration::from_secs_f64(now.videoRefreshPeriod as f64 / now.videoTimeScale as f64));
                let _ = proxy.send_event(AppEvent::Tick{ at, period });
                kCVReturnSuccess
            }
        }

        impl Drop for DisplayLink{
            fn drop(&mut self){
                self.link.stop(); // CFRetained releases the link itself
            }
        }

        pub fn start(proxy:EventLoopProxy<AppEvent>, display_id:u32) -> Option<DisplayLink>{
            unsafe{
                // create the link
                let mut raw:*mut CVDisplayLink = std::ptr::null_mut();
                if CVDisplayLink::create_with_cg_display(display_id, NonNull::from(&mut raw)) != kCVReturnSuccess{
                    return None;
                }
                let link = CFRetained::from_raw(NonNull::new(raw)?);

                // connect the link to our on_vblank callback
                let boxed = Box::new(proxy);
                let ctx = (&*boxed as *const EventLoopProxy<AppEvent>) as *mut c_void;
                if link.set_output_callback(Some(DisplayLink::on_vblank), ctx) != kCVReturnSuccess
                    || link.start() != kCVReturnSuccess
                {
                    return None; // dropping the CFRetained releases the link
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
        use std::time::Duration;
        use windows_sys::Win32::Graphics::Dwm::{DwmFlush, DwmGetCompositionTimingInfo, DwmIsCompositionEnabled, DWM_TIMING_INFO};
        use winit::event_loop::EventLoopProxy;
        use crate::gui::event::AppEvent;
        use super::SourceThread;

        pub fn start(proxy:EventLoopProxy<AppEvent>, fallback:Duration) -> Option<SourceThread>{
            // DwmFlush only tracks vblank while the compositor is running (always on for Win8+)
            let mut enabled:i32 = 0;
            if unsafe{ DwmIsCompositionEnabled(&mut enabled) } != 0 || enabled == 0{
                return None;
            }
            Some(SourceThread::spawn(proxy, move ||{
                // block until the next DWM vblank
                if unsafe{ DwmFlush() } != 0{
                    // if composition drops out mid-run, sleep at the refresh interval so ticks don't stop
                    spin_sleep::sleep(fallback);
                    return None;
                }

                // report the display's current refresh rate so the count follows mode changes
                let mut info = DWM_TIMING_INFO{ cbSize:size_of::<DWM_TIMING_INFO>() as u32, ..Default::default() };
                if unsafe{ DwmGetCompositionTimingInfo(std::ptr::null_mut(), &mut info) } != 0{ return None }
                let rate = info.rateRefresh;
                (rate.uiNumerator > 0 && rate.uiDenominator > 0)
                    .then(|| Duration::from_secs_f64(rate.uiDenominator as f64 / rate.uiNumerator as f64))
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
        use std::time::Duration;
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

            Some(SourceThread::spawn(proxy, move ||{
                // block until the next vblank
                if device.wait_vblank(VblankWaitTarget::Relative(1), VblankWaitFlags::empty(), 0, 0).is_err(){
                    // on error, sleep at the refresh interval so ticks don't stop
                    spin_sleep::sleep(fallback);
                }
                None // drm doesn't report the refresh period
            }))
        }
    }

}
