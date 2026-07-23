#![allow(clippy::upper_case_acronyms)]
use skia_safe::{gpu::DirectContext, ImageInfo, Image, Rect, Matrix, Color, Surface};
use serde_json::{json, Value};
use crate::context::page::{Page, ExportOptions};
use crate::utils::catch_panic;

// reject `window` without a backend also being selected
#[cfg(all(feature = "window", not(any(feature = "metal", feature = "vulkan"))))]
compile_error!("the `window` feature requires enabling `metal` or `vulkan`");

#[cfg(feature = "metal")]
mod metal;
#[cfg(feature = "metal")]
use crate::gfx::metal::offscreen::MetalEngine as Engine;
#[cfg(all(feature = "metal", feature = "window"))]
pub use crate::gfx::metal::window::MetalRenderer as Renderer;

#[cfg(feature = "vulkan")]
mod vulkan;
#[cfg(feature = "vulkan")]
use crate::gfx::vulkan::offscreen::VulkanEngine as Engine;
#[cfg(all(feature = "vulkan", feature = "window"))]
pub use crate::gfx::vulkan::window::VulkanRenderer as Renderer;

// caches for the render thread's gpu-backed page resources: live recording surfaces
// (see context::page::RecordingSurface) and persistent snapshot bitmaps
pub(crate) mod cache;

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
        "error": Value::Null,
    })}
    // placeholders that match the GPU signatures (for the type-checker) but will never be called
    // (see the RenderingEngine methods for their inline implementation when in CPU mode)
    pub fn make_surface(_info: &ImageInfo, _opts:&ExportOptions) -> Result<Surface, String>{ panic!() }
    pub fn with_direct_context(_f:impl FnOnce(Option<&mut DirectContext>)){ panic!() }
    pub fn context_is_idle() -> bool{ false }
    pub fn evict_idle(){ }
    pub fn purge_stale(){ }
    pub fn with_cleanup<T>(f: impl FnOnce() -> T) -> T { f() }
}

// the single thread that serializes jobs bound for the GPU (and its one, shared Context)
mod render_thread{
    use std::cell::Cell;
    use std::panic::{catch_unwind, AssertUnwindSafe};
    use std::sync::{mpsc, OnceLock};
    use std::time::Duration;
    use super::Engine;

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
                                // everything derived from the idle context (cached textures,
                                // recording surfaces) must be released along with it
                                super::cache::evict_idle();
                                Engine::evict_idle();
                            } else {
                                Engine::purge_stale(); // trim oldest entries in skia cache
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
            tx.send(super::catch_panic(f)).ok();
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
}

// fire-and-forget access to the render thread for maintenance of gpu-resident resources
// (cache seeding, registry removal) from threads that shouldn't block on render traffic
pub fn render_soon(f: impl FnOnce() + Send + 'static){
    render_thread::post(f)
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

    pub fn make_surface(&self, image_info: &ImageInfo, opts:&ExportOptions) -> Result<Surface, String>{
        match self {
            Self::GPU => Engine::make_surface(image_info, opts),
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

#[allow(dead_code)]
pub struct RenderCache {
    image: Option<Image>,
    content: Rect,
    page: Page,
    matte: Color,
    dpr: f32,
    state: RenderState,
}

impl Default for RenderCache{
    fn default() -> Self {
        Self{image:None, content:Rect::new_empty(), page:Page::default(), dpr:0.0, matte:Color::TRANSPARENT, state:RenderState::Clean}
    }
}

#[allow(dead_code)]
impl RenderCache{
    pub fn validate(&mut self, page:&Page, matte:Color, dpr:f32, clip:Rect) -> Option<(&Image, &Rect, Rect)>{
        if
            self.state == RenderState::Dirty ||
            self.page.id != page.id ||
            self.matte != matte ||
            self.dpr != dpr
        {
            *self = Self::default();
        }

        self.image.as_ref().map(|img| {
            let (dst, _) = Matrix::scale((dpr, dpr)).map_rect(clip);
            (img, &self.content, dst)
        })
    }

    pub fn depth(&self) -> usize {
        self.page.layers.len()
    }

    pub fn wants_snapshot(&self, page:&Page, matte:Color, dpr:f32, last_id:usize) -> bool{
        // decide (before rendering) whether the frame's snapshot would ever be drawn: skip the
        // GPU→GPU copy when it would just be discarded
        if self.state == RenderState::Resizing{
            return false // update() drops the image during resizes
        }

        let cache_is_current = self.state == RenderState::Clean &&
            self.image.is_some() &&
            self.page.id == page.id &&
            self.matte == matte &&
            self.dpr == dpr;

        match cache_is_current{
            // only re-snapshot when new layers extend the cached content
            true => page.depth() > self.page.depth(),
            // otherwise snapshot only for pages that persist across frames (an id that churns
            // frame-to-frame means full-page redraws that invalidate the cache before reuse)
            false => page.id == last_id
        }
    }

    pub fn update(&mut self, image:Option<Image>, page:&Page, matte:Color, dpr:f32, content:Rect){
        if self.state==RenderState::Resizing{
            // mark the framebuffer as needing a full redraw and skip updating cached image during resize
            self.state = RenderState::Dirty;
        }else if let Some(image) = image{
            let state = RenderState::Clean;
            let (content, _) = skia_safe::Matrix::scale((dpr, dpr)).map_rect(content);
            *self = Self{image: Some(image), page:page.clone(), matte, dpr, content, state};
        }
        // frames without a snapshot leave any cached image in place (it's still a valid prefix
        // of the current page)
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum RenderState{
    Clean,
    Dirty,
    Resizing
}

#[cfg(feature = "window")] // only the windowed renderers produce these
pub enum RenderOutcome {
    Skipped, // surface wasn't available in time so couldn't redraw
    Rendered(Option<Image>), // succeded (including snapshot if requested)
}
