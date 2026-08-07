#![allow(clippy::upper_case_acronyms)]
use skia_safe::{gpu::DirectContext, ImageInfo, Surface};
#[cfg(feature = "window")]
use skia_safe::Image; // RenderOutcome carries a snapshot destined for the Frame cache
use serde_json::{json, Value};
use crate::gfx::{self, page::ExportOptions};
use crate::utils::catch_panic;

#[cfg(feature = "metal")]
use crate::gfx::metal::offscreen::MetalEngine as Engine;
#[cfg(feature = "vulkan")]
use crate::gfx::vulkan::offscreen::VulkanEngine as Engine;

#[cfg(not(any(feature = "vulkan", feature = "metal")))]
struct Engine { }
#[cfg(not(any(feature = "vulkan", feature = "metal")))]
impl Engine {
    pub fn api() -> Option<String>{ None }
    pub fn supported() -> bool { false }
    pub fn status() -> Value { serde_json::json!({
        "renderer": "CPU",
        "api": Value::Null,
        "device": "CPU-based renderer (compiled without GPU support)",
        "threads": rayon::current_num_threads(),
        "error": Value::Null,
    })}
    // placeholders that match the GPU signatures (for the type-checker) but will never be called
    // (see the RenderingEngine methods for their inline implementation when in CPU mode)
    pub fn make_surface(_info: &ImageInfo, _opts:&ExportOptions, _budgeted:bool) -> Result<Surface, String>{ panic!() }
    pub fn with_direct_context(_f:impl FnOnce(Option<&mut DirectContext>)){ panic!() }
    pub fn context_is_idle() -> bool{ false }
    pub fn retire(){ }
    pub fn purge_stale(){ }
    pub fn with_cleanup<T>(f: impl FnOnce() -> T) -> T { f() }
}

#[derive(Copy, Clone, Debug)]
pub enum RenderingEngine{
    CPU,
    GPU,
}

impl Default for RenderingEngine {
    fn default() -> Self {
        if Engine::supported() { Self::GPU } else { Self::CPU }
    }
}

#[allow(dead_code)]
impl RenderingEngine{
    pub fn selectable(&self) -> bool {
        match self {
            Self::GPU => Engine::supported(),
            Self::CPU => true
        }
    }

    pub fn make_surface(&self, image_info: &ImageInfo, opts:&ExportOptions, budgeted:bool) -> Result<Surface, String>{
        match self {
            Self::GPU => Engine::make_surface(image_info, opts, budgeted),
            Self::CPU => skia_safe::surfaces::raster(image_info, None, Some(&opts.surface_props()))
                .ok_or(format!("Could not allocate new {}×{} bitmap", image_info.width(), image_info.height()))
        }
    }

    // run a closure on the rendering thread (GPU) or current thread (CPU) and convert panics
    // into Err values that can be sent back to js as promise rejections
    pub fn render<T, F>(&self, f:F) -> Result<T, String>
        where F:FnOnce() -> Result<T, String> + Send + 'static, T:Send + 'static
    {
        match self {
            Self::GPU => render_thread::run(f),
            Self::CPU => catch_panic(f)
        }
    }

    pub fn with_direct_context(&self, f:impl FnOnce(Option<&mut DirectContext>)){
        match self {
            Self::GPU => Engine::with_direct_context(f),
            Self::CPU => f(None)
        }
    }

    pub fn status(&self, is_manually_disabled:bool) -> serde_json::Value {
        match is_manually_disabled{
            true => json!({
                "renderer":"CPU",
                "api": Engine::api(),
                "device": "CPU-based renderer (GPU manually disabled)",
                "driver": "N/A",
                "threads": rayon::current_num_threads()
            }),
            false=> Engine::status(),
        }
    }

    pub fn lacks_gpu_support(&self) -> Option<String> {
        match Engine::supported(){
            true => None,
            false => {
                let mut msg = vec!["No windowing support".to_string()];
                if let Some(Value::String(error)) = Engine::status().get("error"){
                    msg.push(error.to_string());
                }
                Some(msg.join(": "))
            }
        }
    }
}

#[cfg(feature = "window")] // only the windowed renderers produce these
pub enum RenderOutcome {
    Skipped, // surface wasn't available in time so couldn't redraw
    Rendered(Option<Image>), // succeded (including snapshot if requested)
}

// the single thread that serializes jobs bound for the GPU (and its one, shared Context)
mod render_thread{
    use std::cell::Cell;
    use std::panic::{catch_unwind, AssertUnwindSafe};
    use std::sync::{mpsc, OnceLock};
    use std::time::Duration;
    use super::{gfx, Engine};

    type Job = Box<dyn FnOnce() + Send>;
    static SENDER: OnceLock<mpsc::Sender<Job>> = OnceLock::new();
    thread_local!( static IS_RENDER_THREAD: Cell<bool> = const { Cell::new(false) }; );

    fn sender() -> &'static mpsc::Sender<Job>{
        SENDER.get_or_init(||{
            let (tx, rx) = mpsc::channel::<Job>();
            std::thread::spawn(move ||{
                IS_RENDER_THREAD.set(true);
                loop{
                    match rx.recv_timeout(Duration::from_secs(1)){
                        // jobs from post() have no return channel, so their errors are printed to
                        // stderr by the default hook then caught here so the thread keeps running.
                        // jobs from run() catch their own panics and relay an Err through their rx.
                        Ok(job) => { Engine::with_cleanup(|| catch_unwind(AssertUnwindSafe(job)).ok()); },
                        Err(mpsc::RecvTimeoutError::Timeout) => {
                            if Engine::context_is_idle(){
                                gfx::cache::evict_idle(); // free recording surfaces and snapshots
                                Engine::retire(); // drop the context
                            } else {
                                Engine::purge_stale(); // trim oldest entries in skia's internal cache
                            }
                        },
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                }
            });
            tx
        })
    }

    // run closure on the render thread and block until results arrive on response channel
    pub fn run<T, F>(f:F) -> Result<T, String>
        where F:FnOnce() -> Result<T, String> + Send + 'static, T:Send + 'static
    {
        if IS_RENDER_THREAD.get(){
            return f() // don't deadlock on re-entrant calls from within a render job
        }
        let (tx, rx) = mpsc::channel();
        sender().send(Box::new(move || {
            tx.send(crate::utils::catch_panic(f)).ok();
        })).expect("Render thread unavailable");
        rx.recv().unwrap_or_else(|_| Err("Render thread unavailable".to_string()))
    }

    // send a closure to the render thread without waiting for a response (primarily used
    // for performing deallocations of gpu objects on the same thread as the context)
    pub fn post<F>(f:F)
        where F:FnOnce() + Send + 'static
    {
        if IS_RENDER_THREAD.get(){
            return f()
        }
        if let Some(tx) = SENDER.get(){
            tx.send(Box::new(f)).ok();
        }
    }

    pub fn is_running() -> bool { SENDER.get().is_some() }
}

// fire-and-forget access to the render thread for maintenance of gpu-resident resources
// (cache seeding, registry removal) from threads that shouldn't block on render traffic
pub fn render_soon(f: impl FnOnce() + Send + 'static){
    render_thread::post(f)
}

// retire the gpu context before process shutdown (on the render thread where it lives) so
// its destructor can run while the driver is still alive. otherwise the thread-local destructor
// would run at dll-detach on Windows, when it's too late to cleanly shut down.
pub fn retire_gpu(){
    if !render_thread::is_running(){ return }
    render_thread::run(|| {
        gfx::cache::evict_idle(); // free recording surfaces and snapshots
        Engine::retire(); // drop the context
        Ok::<(), String>(())
    }).ok();
}
