use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::OnceLock;
use neon::prelude::*;

// a neon channel used for sending scheduled flushes back to node, keeping its memory estimate up-to-date
static CHANNEL: OnceLock<Channel> = OnceLock::new();

pub fn install_channel<'a, C: Context<'a>>(cx: &mut C) {
    let mut channel = Channel::new(cx);
    channel.unref(cx);
    let _ = CHANNEL.set(channel);
}

// the process-wide v8 external-memory ledger
static ACCOUNTING: Accounting = Accounting::new();

pub struct Accounting {
    pending: AtomicI64, // net change in allocated bytes (to be flushed to v8)
    scheduled: AtomicBool, // flag when a channel flush has been enqueued
}

impl Accounting {
    const fn new() -> Self {
        Self { pending: AtomicI64::new(0), scheduled: AtomicBool::new(false) }
    }

    // update the `pending` byte delta: + on allocation, - when it's freed and enqueue a flush
    pub fn charge(&self, delta: i64) {
        if delta != 0 {
            self.pending.fetch_add(delta, Ordering::Relaxed);
            self.schedule_flush();
        }
    }

    // enqueue a channel flush, coalescing subsequent requests until it fires
    fn schedule_flush(&self) {
        // only start a new flush if one wasn't already scheduled
        if !self.scheduled.swap(true, Ordering::AcqRel) {
            CHANNEL.get()
                .and_then(|ch| ch.try_send(|cx| {
                    // clear the flag *before* flushing to v8, so any updates that arrive
                    // concurrently re-enqueue and aren't left stranded
                    ACCOUNTING.scheduled.store(false, Ordering::Release);
                    NAPI.adjust_external_memory(&cx, ACCOUNTING.pending.swap(0, Ordering::Relaxed));
                    Ok(())
                }).ok())
                .or_else(|| {
                    // the channel send failed: release the flag so it can be retried later
                    self.scheduled.store(false, Ordering::Release);
                    None
                });
        }
    }
}

// wrapper for the napi_adjust_external_memory call (finds the raw fn via libloading)
static NAPI: Napi = Napi::new();

struct Napi {
    adjust: OnceLock<Option<unsafe extern "C" fn(*mut c_void, i64, *mut i64) -> i32>>,
}

impl Napi {
    const fn new() -> Self {
        Self { adjust: OnceLock::new() }
    }

    // find the (platform-specific) handle to the process's symbol table
    fn host_lib() -> Option<libloading::Library> {
        #[cfg(windows)] // fallible on windows
        let lib = libloading::os::windows::Library::this().ok().map(Into::into);
        #[cfg(not(windows))] // always works on unix
        let lib = Some(libloading::os::unix::Library::this().into());
        lib
    }

    // update v8 if the current delta is non-zero
    fn adjust_external_memory<'a, C: Context<'a>>(&self, cx: &C, delta: i64) {
        if delta != 0 {
            // resolve (and memoize) the raw napi fn pointer
            self.adjust.get_or_init(|| {
                let host = Self::host_lib()?;
                let sym = unsafe { host.get(b"napi_adjust_external_memory") }.ok()?;
                Some(*sym)
            }).map(|napi_adjust_external_memory| unsafe {
                // call it to update napi's external memory size estimate
                napi_adjust_external_memory(cx.to_raw() as *mut c_void, delta, &mut 0i64);
            });
        }
    }
}

// RAII accounting token. adds to the v8 memory budget on creation and subtracts from it
// on drop. must not support Clone (since that would double-count the same allocation)
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
            ACCOUNTING.charge(bytes as i64 - self.0 as i64);
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
            ACCOUNTING.charge(-(self.0 as i64));
        }
    }
}
