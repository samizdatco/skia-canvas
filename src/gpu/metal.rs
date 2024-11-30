use std::cell::RefCell;
use std::sync::{Arc, OnceLock};
use std::time::{Instant, Duration};
use cocoa::{appkit::NSView, base::id as cocoa_id};
use core_graphics_types::geometry::CGSize;
use metal::{
    CommandQueue, Device, MTLPixelFormat, MetalLayer, MTLDeviceLocation,
    foreign_types::{ForeignType, ForeignTypeRef}
};
use skia_safe::{scalar, ImageInfo, ColorType, Size, Surface};
use skia_safe::gpu::{
    mtl, direct_contexts, backend_render_targets, surfaces, Budgeted, DirectContext, SurfaceOrigin
};
use objc::{runtime::YES, rc::autoreleasepool};
use serde_json::{json, Value};
use crossbeam::channel;

thread_local!( static MTL_CONTEXT: RefCell<Option<MetalContext>> = const { RefCell::new(None) }; );
static MTL_CONTEXT_LIFESPAN:Duration = Duration::from_secs(5);
static MTL_STATUS: OnceLock<Value> = OnceLock::new();

//
// Offscreen rendering
//
pub struct MetalEngine {}

impl MetalEngine {
    pub fn supported() -> bool {
        Self::status()["renderer"] == "GPU"
    }

    pub fn status() -> Value {
        MTL_STATUS.get_or_init(||{
            // test whether a context can be created and do some one-time init if so
            match MetalContext::new(){
                Some(context) => {
                    Self::spawn_idle_watcher(); // watch for inactive contexts and deallocate them

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

    fn spawn_idle_watcher(){
        // use a non-rayon thread so as not to compete with the worker threads
        std::thread::spawn(move || loop{
            // run forever, watching the other threads in the pool
            std::thread::sleep(Duration::from_secs(1));
            rayon::spawn_broadcast(|_|{
                // drop contexts that haven't been used in a while to free resources
                MTL_CONTEXT.with_borrow_mut(|cell| {
                    cell.take_if(|engine|{
                        engine.cleanup(); // it's unclear how effective this is...
                        engine.last_use.elapsed() > MTL_CONTEXT_LIFESPAN
                    });
                });
            });
        });
    }

    pub fn with_surface<T, F>(image_info: &ImageInfo, msaa:Option<usize>, f:F) -> Result<T, String>
        where F:FnOnce(&mut Surface) -> Result<T, String>
    {
        match MetalEngine::supported() {
            false => Err("Metal API not supported".to_string()),
            true => MTL_CONTEXT.with_borrow_mut(|local_ctx|
                autoreleasepool(||
                    local_ctx
                        // lazily initialize this thread's context...
                        .take()
                        .or_else(|| MetalContext::new() )
                        .ok_or("Metal initialization failed".to_string())
                        .and_then(|ctx|{
                            let ctx = local_ctx.insert(ctx);
                            // ...then create the surface with it...
                            ctx.surface(image_info, msaa)
                        })
                        .and_then(|mut surface|
                            // ... finally let the callback use it
                            f(&mut surface)
                        )
                )
            )
        }
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
                let queue = device.new_command_queue();
                let backend = unsafe {
                    mtl::BackendContext::new(
                        device.as_ptr() as mtl::Handle,
                        queue.as_ptr() as mtl::Handle,
                    )
                };
                let last_use = Instant::now() + MTL_CONTEXT_LIFESPAN;
                let msaa:Vec<usize> = [0,2,4,8,16,32].into_iter().filter(|s|{
                    *s==0 || device.supports_texture_sample_count(*s as _)
                }).collect();
                direct_contexts::make_metal(&backend, None)
                    .map(|context| MetalContext{device, context, msaa, last_use})
            })
        })
    }

    fn surface(&mut self, image_info: &ImageInfo, msaa:Option<usize>) -> Result<Surface, String> {
        let samples = msaa.unwrap_or_else(||
            if self.msaa.contains(&4){ 4 } // 4x is a good default if available
            else{ *self.msaa.last().unwrap() }
        );
        if !self.msaa.contains(&samples){
            return Err(format!("{}x MSAA not supported by GPU (options: {:?})", samples, self.msaa));
        }

        self.last_use = self.last_use.max(Instant::now());
        surfaces::render_target(
            &mut self.context,
            Budgeted::Yes,
            image_info,
            Some(samples),
            SurfaceOrigin::BottomLeft,
            None,
            false,
            None
        ).ok_or(
            format!("Could not allocate new {}×{} bitmap", image_info.width(), image_info.height())
        )
    }

    fn cleanup(&mut self){
        self.context.free_gpu_resources();
        self.context.perform_deferred_cleanup(Duration::from_secs(1), None);
    }
}

//
// Windowed rendering
//

#[cfg(feature = "window")]
use {
    crate::{
        context::page::Page,
        gui::event::GpuEvent,
    },
    winit::{
        dpi::PhysicalSize,
        window::Window,
        raw_window_handle::HasWindowHandle,
        event_loop::ActiveEventLoop,
    },
    skia_safe::{Matrix, Color},
};

#[cfg( feature = "window")]
pub struct MetalRenderer {
    backend: channel::Sender<GpuEvent>,
}

#[cfg( feature = "window")]
impl MetalRenderer{
    pub fn for_window(_event_loop: &ActiveEventLoop, window:Arc<Window>) -> Self {
        let device = Device::system_default().expect("no device found");

        let raw_window_handle = window
            .window_handle()
            .expect("Failed to retrieve a window handle")
            .as_raw();

        let layer = {
            let draw_size = window.inner_size();
            let layer = MetalLayer::new();
            layer.set_device(&device);
            layer.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
            layer.set_presents_with_transaction(false);
            layer.set_opaque(false);
            layer.set_framebuffer_only(false); // to enable blend modes

            unsafe {
                let view = match raw_window_handle {
                    raw_window_handle::RawWindowHandle::AppKit(appkit) => {
                        appkit.ns_view.as_ptr()
                    }
                    _ => panic!("Wrong window handle type"),
                } as cocoa_id;
                view.setWantsLayer(YES);
                view.setLayer(layer.as_ref() as *const _ as _);
            }
            layer.set_drawable_size(CGSize::new(draw_size.width as f64, draw_size.height as f64));
            layer
        };

        let dpr = window.scale_factor();

        // spawn a background thread where the Backend will wait for new pages via its rx channel
        let (tx, rx) = channel::unbounded();
        std::thread::spawn(move || {
            let mut backend = MetalBackend::for_layer(&layer);
            while let Ok(event) = rx.recv() {
                if !rx.is_empty(){ continue } // drop all but the last frame in the queue

                autoreleasepool(||{
                    match event{
                        GpuEvent::Resize(size) => {
                            let cg_size = CGSize::new(size.width as f64, size.height as f64);
                            layer.set_drawable_size(cg_size);
                        },
                        GpuEvent::Draw(page, matrix, matte) => {
                            let (clip, _) = matrix.map_rect(page.bounds);
                            let scale = Matrix::scale((dpr as f32, dpr as f32));
                            backend.render_to_layer(&layer, |canvas|{
                                canvas.clear(matte)
                                    .set_matrix(&scale.into())
                                    .clip_rect(clip, None, Some(true))
                                    .draw_picture(page.get_picture(None).unwrap(), Some(&matrix), None);
                            }).unwrap();
                        }
                    }
                })
            }
        });

        Self{backend:tx}
    }

    pub fn resize(&self, size: PhysicalSize<u32>) {
        self.backend.send( GpuEvent::Resize(size) ).ok();
    }

    pub fn draw(&self, page:Page, matrix:Matrix, matte:Color){
        self.backend.send( GpuEvent::Draw(page, matrix, matte) ).ok();
    }
}

pub struct MetalBackend {
    // each renderer's non-Send references need to be lazily allocated on the window's thread
    skia_ctx: DirectContext,
    queue: CommandQueue,
}

impl Drop for MetalBackend{
    fn drop(&mut self) {
        self.skia_ctx.abandon();
    }
}

impl MetalBackend {
    pub fn for_layer(layer:&MetalLayer) -> Self{
        let queue = layer.device().new_command_queue();
        let backend_ctx = unsafe {
            mtl::BackendContext::new(
                layer.device().as_ptr() as mtl::Handle,
                queue.as_ptr() as mtl::Handle,
            )
        };
        let skia_ctx = direct_contexts::make_metal(&backend_ctx, None).unwrap();
        Self { skia_ctx, queue }
    }

    fn render_to_layer<F>(&mut self, layer:&MetalLayer, f:F) -> Result<(), String>
        where F:FnOnce(&skia_safe::Canvas)
    {
        let drawable = layer
            .next_drawable()
            .ok_or("MetalBackend: could not allocate framebuffer".to_string())?;

        let drawable_size = {
            let size = layer.drawable_size();
            Size::new(size.width as scalar, size.height as scalar)
        };

        let backend_render_target = unsafe {
            let texture_info =
                mtl::TextureInfo::new(drawable.texture().as_ptr() as mtl::Handle);
            backend_render_targets::make_mtl(
                (drawable_size.width as i32, drawable_size.height as i32),
                &texture_info,
            )
        };

        let mut surface = surfaces::wrap_backend_render_target(
            &mut self.skia_ctx,
            &backend_render_target,
            SurfaceOrigin::TopLeft,
            ColorType::BGRA8888,
            None,
            None,
        ).ok_or("MetalBackend: could not create render target")?;

        f(surface.canvas());

        self.skia_ctx.flush_and_submit();
        self.skia_ctx.free_gpu_resources();

        let command_buffer = self.queue.new_command_buffer();
        command_buffer.present_drawable(drawable);
        command_buffer.commit();
        Ok(())
    }

}