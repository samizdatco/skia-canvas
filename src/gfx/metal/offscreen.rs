use std::cell::RefCell;
use std::sync::OnceLock;
use std::time::{Instant, Duration};
use metal::{ Device, MTLDeviceLocation };
use skia_safe::{ImageInfo, Surface};
use skia_safe::gpu::{ surfaces, Budgeted, DirectContext, SurfaceOrigin };
use objc::rc::autoreleasepool;
use serde_json::{json, Value};

use crate::context::page::ExportOptions;
use super::make_direct_context;

thread_local!( static MTL_CONTEXT: RefCell<Option<MetalContext>> = const { RefCell::new(None) }; );
static MTL_CONTEXT_LIFESPAN:Duration = Duration::from_millis(2500); // rebuild is cheap, so use a short lifespan
static MTL_STATUS: OnceLock<Value> = OnceLock::new();

//
// Offscreen rendering
//
pub struct MetalEngine;

impl MetalEngine {
    pub fn api() -> Option<String>{
        Some("Metal".to_string())
    }

    pub fn supported() -> bool {
        Self::status()["renderer"] == "GPU"
    }

    pub fn status() -> Value {
        MTL_STATUS.get_or_init(||{
            // test whether a context can be created and do some one-time init if so
            match MetalContext::new(){
                Some(context) => {
                    let device_name = format!("{} ({})", match context.device.location(){
                        MTLDeviceLocation::BuiltIn => "Integrated GPU",
                        MTLDeviceLocation::Slot => "Discrete GPU",
                        MTLDeviceLocation::External => "External GPU",
                        _ => "Other GPU"
                    }, context.device.name());

                    json!({
                        "renderer": "GPU",
                        "api": "Metal",
                        "device": device_name,
                        "threads": rayon::current_num_threads(),
                    })
                }
                None => json!({
                    "renderer": "CPU",
                    "api": "Metal",
                    "device": "CPU-based renderer (Fallback)",
                    "threads": rayon::current_num_threads(),
                    "error": "GPU initialization failed",
                })
            }
        }).clone()
    }

    pub fn with_context<T, F>(f:F) -> Result<T, String>
        where F:FnOnce(&mut MetalContext) -> Result<T, String>
    {
        match MetalEngine::supported() {
            false => Err("Metal API not supported".to_string()),
            true => MTL_CONTEXT.with_borrow_mut(|local_ctx|
                autoreleasepool(||
                    // lazily initialize this thread's context...
                    local_ctx
                        .take()
                        .or_else(|| MetalContext::new() )
                        .ok_or("Metal initialization failed".to_string())
                        .and_then(|ctx|{
                            f(local_ctx.insert(ctx))
                        })
                )
            )
        }
    }

    pub fn with_direct_context<F>(f:F)
        where F:FnOnce(Option<&mut DirectContext>)
    {
        Self::with_context(|ctx| Ok(f(Some(&mut ctx.context))) ).ok();
    }

    pub fn make_surface(image_info: &ImageInfo, opts:&ExportOptions) -> Result<Surface, String>{
        Self::with_context(|ctx| ctx.surface(image_info, opts) )
    }

    // allow the render thread to check how long the context has gone unused
    pub fn context_is_idle() -> bool{
        MTL_CONTEXT.with_borrow(|cell|
            cell.as_ref().map(|engine| engine.last_use.elapsed() > MTL_CONTEXT_LIFESPAN).unwrap_or(false)
        )
    }

    // called by the render thread when idle to drop the context & free gpu resources
    pub fn evict_idle(){
        MTL_CONTEXT.with_borrow_mut(|cell| {
            cell.take_if(|engine| engine.last_use.elapsed() > MTL_CONTEXT_LIFESPAN);
        });
    }

    // called by the render thread between jobs to expire skia's internal caches
    pub fn purge_stale(){
        MTL_CONTEXT.with_borrow_mut(|cell| {
            if let Some(engine) = cell.as_mut(){
                autoreleasepool(||
                    engine.context.perform_deferred_cleanup(Duration::from_secs(1), None)
                );
            }
        });
    }

    // create a job-specific autorelease pool for each pass through the render-thread
    pub fn with_cleanup<T>(f: impl FnOnce() -> T) -> T {
        autoreleasepool(f)
    }
}

pub struct MetalContext {
    device: Device,
    context: DirectContext,
    msaa: Vec<usize>,
    last_use: Instant,
}

impl MetalContext{
    fn new() -> Option<Self>{
        autoreleasepool(|| {
            Device::system_default().and_then(|device|{
                let last_use = Instant::now() + MTL_CONTEXT_LIFESPAN;
                let msaa:Vec<usize> = [0,2,4,8,16,32].into_iter().filter(|s|{
                    *s==0 || device.supports_texture_sample_count(*s as _)
                }).collect();
                make_direct_context(&device)
                    .map(|(_queue, context)| MetalContext{device, context, msaa, last_use})
            })
        })
    }

    fn surface(&mut self, image_info: &ImageInfo, opts:&ExportOptions) -> Result<Surface, String> {
        self.last_use = self.last_use.max(Instant::now());
        surfaces::render_target(
            &mut self.context,
            Budgeted::Yes,
            image_info,
            Some(opts.msaa_from(&self.msaa)?),
            SurfaceOrigin::BottomLeft,
            Some(&opts.surface_props()),
            false,
            None
        ).ok_or(
            format!("Could not allocate new {}×{} bitmap", image_info.width(), image_info.height())
        )
    }

}
