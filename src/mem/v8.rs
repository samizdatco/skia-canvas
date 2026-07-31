use std::ffi::c_void;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::OnceLock;
use neon::prelude::*;

// Net native bytes charged to V8 but not yet flushed, will be reset to 0 at the next flush
static PENDING: AtomicI64 = AtomicI64::new(0);

// napi_adjust_external_memory(env, change_in_bytes, *adjusted_value) -> napi_status
type AdjustFn = unsafe extern "C" fn(*mut c_void, i64, *mut i64) -> i32;
type Adjust = dyn Fn(*mut c_void, i64) + Send + Sync;

// Run `f` with the resolved adjuster (or do nothing if it isn't available)
#[cfg(unix)]
fn with_adjust(f: impl FnOnce(&Adjust)) {
    static F: OnceLock<Option<Box<Adjust>>> = OnceLock::new();
    let adjust = F.get_or_init(|| {
        // use RTLD_DEFAULT to search loaded objects and find the napi symbol
        let name = c"napi_adjust_external_memory";
        let p = unsafe { libc::dlsym(libc::RTLD_DEFAULT, name.as_ptr()) };
        let adjust: AdjustFn = (!p.is_null())
            .then(|| unsafe { std::mem::transmute::<usize, AdjustFn>(p as usize) })?;
        // store an `adjust` wrapper that handles the out-param napi expects
        Some(Box::new(move |env, delta| {
            let mut adjusted = 0i64;
            unsafe { adjust(env, delta, &mut adjusted) };
        }))
    });
    if let Some(adjust) = adjust {
        f(adjust);
    }
}

// Windows would need GetProcAddress rather than dlsym; not wired, so accounting is inactive.
#[cfg(not(unix))]
fn with_adjust(_f: impl FnOnce(&Adjust)) {}

// Update the pending byte delta: + on allocation, - when it's freed
pub fn charge(delta: i64) {
    if delta == 0 {
        return;
    }
    PENDING.fetch_add(delta, Ordering::Relaxed);
}

// pass the accumulated delta to V8. requires a cx reference and must run on the main thread
// (cf. the changes to PENDING which can happen from any thread)
pub fn flush<'a, C: Context<'a>>(cx: &mut C) {
    with_adjust(|adjust| {
        let delta = PENDING.swap(0, Ordering::Relaxed);
        if delta != 0 {
            adjust(cx.to_raw() as *mut c_void, delta);
        }
    });
}

// RAII accounting token. adds to the v8 memory budget on creation and deducts from it
// on drop. the byte-count can also be updated after-the-fact with `.set()` (in case
// incremental allocations/frees happen later). does not support Clone (because that would
// double-count the same allocation).
#[derive(Default, Debug)]
pub struct Footprint(usize);

impl Footprint {
    // create a token already holding `bytes`
    pub fn new(bytes: usize) -> Self {
        let mut c = Self::default();
        c.set(bytes);
        c
    }

    // manually set the number of native bytes represented
    pub fn set(&mut self, bytes: usize) {
        if bytes != self.0 {
            charge(bytes as i64 - self.0 as i64);
            self.0 = bytes;
        }
    }

    // mark the allocation as having been fully released
    pub fn clear(&mut self) {
        self.set(0);
    }
}

// automatically clear on drop
impl Drop for Footprint {
    fn drop(&mut self) {
        if self.0 != 0 {
            charge(-(self.0 as i64));
        }
    }
}
