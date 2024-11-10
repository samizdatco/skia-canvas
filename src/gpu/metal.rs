#![allow(dead_code)]
#![allow(unused_imports)]
use std::cell::RefCell;
use std::sync::{Arc, OnceLock};
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
use objc::runtime::YES;
pub use objc::rc::autoreleasepool;
use serde_json::{json, Value};

#[cfg(feature = "window")]
use winit::{
    dpi::{LogicalSize, PhysicalSize},
    platform::macos::WindowExtMacOS,
    window::Window,
    raw_window_handle::HasWindowHandle,
    event_loop::ActiveEventLoop,
};

thread_local!(
    static MTL_CONTEXT: RefCell<Option<MetalContext>> = const { RefCell::new(None) };
);

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
                        "device": device_name
                    })        
                }
                None => json!({
                    "renderer": "CPU",
                    "api": "Metal",
                    "device": "CPU-based renderer (Fallback)",
                    "error": "GPU initialization failed",
                })
            }
        }).clone()
    }

    pub fn surface(image_info: &ImageInfo) -> Option<Surface> {
        match MetalEngine::supported() {
            false => None,
            true => MTL_CONTEXT.with_borrow_mut(|local_ctx| {
                // lazily initialize this thread's context...
                local_ctx
                    .take()
                    .or_else(|| MetalContext::new() )
                    .and_then(|ctx|{
                        let ctx = local_ctx.insert(ctx);
                        // ...then create the surface with it
                        ctx.surface(image_info)
                    })
            })
        }
    }
}
pub struct MetalContext {
    device: Device,
    queue: CommandQueue,
    context: DirectContext,
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
                direct_contexts::make_metal(&backend, None)
                    .map(|context| MetalContext{device, queue, context})
            })
        })
    }

    fn surface(&mut self, image_info: &ImageInfo) -> Option<Surface> {
        surfaces::render_target(
            &mut self.context,
            Budgeted::Yes,
            image_info,
            Some(4),
            SurfaceOrigin::BottomLeft,
            None,
            false,
            None
        )
    }
}

// 
// Windowed rendering
// 

pub struct MetalRenderer {
    layer: Arc<MetalLayer>,
    device: Arc<Device>,
}

// The windowed renderer
impl MetalRenderer {
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

        Self { layer: Arc::new(layer), device: Arc::new(device) }
    }

    pub fn resize(&self, size: PhysicalSize<u32>) {
        let cg_size = CGSize::new(size.width as f64, size.height as f64);
        self.layer.set_drawable_size(cg_size);
    }

    pub fn draw<F>(
        &mut self,
        window: &Arc<Window>,
        f: F,
    ) -> Result<(), String> 
        where F:FnOnce(&skia_safe::Canvas, LogicalSize<f32>)
    {
        let dpr = window.scale_factor();
        let size = window.inner_size();
        BACKEND.with_borrow_mut(|cell| {
            let backend = cell.get_or_insert_with(|| MetalBackend::for_renderer(self));

            backend.render_to_layer(&self.layer, |canvas|{
                canvas.reset_matrix();
                canvas.scale((dpr as f32, dpr as f32));
                f(canvas, LogicalSize::from_physical(size, dpr));
            })
        })
    }
}

impl Drop for MetalRenderer {
    fn drop(&mut self) {
        BACKEND.with_borrow_mut(|cell| *cell = None );
    }
}


thread_local!(static BACKEND: RefCell<Option<MetalBackend>> = const { RefCell::new(None) } );

pub struct MetalBackend {
    // each renderer's non-Send references need to be lazily allocated on the window's thread
    skia_ctx: DirectContext,
    queue: CommandQueue,
}

#[cfg(feature = "window")]
impl MetalBackend {
    pub fn for_renderer(renderer:&MetalRenderer) -> Self {
        let queue = renderer.device.new_command_queue();

        let backend_ctx = unsafe {
            mtl::BackendContext::new(
                renderer.device.as_ptr() as mtl::Handle,
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