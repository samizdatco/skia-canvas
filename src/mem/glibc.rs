// glibc malloc has trouble reclaiming freed allocations when the top of its arena still
// contains scattered live objects. malloc_trim defragments & forces the release, but its cost
// scales roughly with the amount of memory reclaimed and is actively counterproductive if it runs
// during a burst of activity. a background thread watches for idle periods in which the amount of
// known-to-be-reclaimable memory is above its threshold and runs malloc_trim() once it's worthwhile.
//
// setting the SKIA_CANVAS_TRIM environment var to `off`/`0`/`false` will disable this behavior.
// or it can be set to `eager` to make it run whenever there's a lull in activity.

#[cfg(all(target_os = "linux", target_env = "gnu"))]
mod heap_watcher {
    use std::sync::{Condvar, Mutex, OnceLock};
    use std::thread;
    use std::time::{Duration, Instant};

    unsafe extern "C" {
        fn malloc_trim(pad: usize) -> std::os::raw::c_int;
    }

    // only run if at least this much render-allocated garbage had piled up (as measured by PageRecorder's accounting)
    const MIN_BYTES: u64 = 4 * 1024 * 1024;

    // SKIA_CANVAS_TRIM=eager lowers the threshold so trim will run (nearly) every time the idle timer fires
    const EAGER_MIN_BYTES: u64 = 1024 * 1024;

    // wait this long after the last PageRecorder.release() (to avoid trimming in the middle of a burst)
    const IDLE_DEBOUNCE: Duration = Duration::from_millis(1500);

    // returns the active min-bytes threshold, or None if opted out via SKIA_CANVAS_TRIM=off
    fn config() -> Option<u64> {
        static CONFIG: OnceLock<Option<u64>> = OnceLock::new();
        *CONFIG.get_or_init(|| {
            let setting = std::env::var("SKIA_CANVAS_TRIM")
                .map(|v| v.trim().to_ascii_lowercase())
                .unwrap_or_default();
            Some(match setting.as_str() {
                "0" | "false" | "off" => return None,
                "eager" => EAGER_MIN_BYTES,
                _ => MIN_BYTES,
            })
        })
    }

    struct State {
        bytes_since_trim: u64,
        last_activity: Instant,
    }

    fn state() -> &'static (Mutex<State>, Condvar) {
        static PAIR: OnceLock<(Mutex<State>, Condvar)> = OnceLock::new();
        PAIR.get_or_init(|| {
            (Mutex::new(State { bytes_since_trim: 0, last_activity: Instant::now() }), Condvar::new())
        })
    }

    fn ensure_worker() {
        static STARTED: OnceLock<()> = OnceLock::new();
        STARTED.get_or_init(|| {
            let spawned = thread::Builder::new().name("skia-canvas-trim".into()).spawn(worker_loop);
            if let Err(err) = spawned {
                eprintln!("skia-canvas: failed to start allocator-trim thread: {err}");
            }
        });
    }

    fn worker_loop() {
        let min_bytes = config().expect("trim worker only spawns when enabled");
        let (mutex, condvar) = state();
        let mut guard = mutex.lock().unwrap();
        loop {
            let (g, _) = condvar.wait_timeout(guard, IDLE_DEBOUNCE).unwrap();
            guard = g;

            let due = guard.bytes_since_trim >= min_bytes && guard.last_activity.elapsed() >= IDLE_DEBOUNCE;
            if due {
                guard.bytes_since_trim = 0;
                drop(guard); // never hold the lock across the (potentially many-ms) trim call
                unsafe { malloc_trim(0); }
                guard = mutex.lock().unwrap();
            }
        }
    }

    pub fn mark_reclaimable(estimated_bytes: usize) {
        if config().is_none() { return } // SKIA_CANVAS_TRIM=off opt-out
        ensure_worker();
        let (mutex, condvar) = state();
        {
            let mut guard = mutex.lock().unwrap();
            guard.bytes_since_trim = guard.bytes_since_trim.saturating_add(estimated_bytes as u64);
            guard.last_activity = Instant::now();
        }
        condvar.notify_one();
    }
}

// only necessary on glibc (macOS, Windows, and musl don't have the same pathology)
#[cfg(all(target_os = "linux", target_env = "gnu"))]
pub use heap_watcher::mark_reclaimable;

#[cfg(not(all(target_os = "linux", target_env = "gnu")))]
pub fn mark_reclaimable(_estimated_bytes: usize) {}
