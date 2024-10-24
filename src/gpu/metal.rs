#![allow(unused_imports)]
#![allow(dead_code)]

use std::cell::RefCell;
use std::sync::{Arc, Mutex};
use foreign_types_shared::{ForeignType, ForeignTypeRef};
use cocoa::{appkit::NSView, base::id as cocoa_id};
use core_graphics_types::geometry::CGSize;
use metal::{CommandQueue, Device, MTLPixelFormat, MetalLayer};
use skia_safe::{scalar, ImageInfo, ColorType, Size, Surface};
use skia_safe::gpu::{
    mtl, direct_contexts, backend_render_targets, surfaces, Budgeted, DirectContext, SurfaceOrigin
};
use objc::runtime::YES;
pub use objc::rc::autoreleasepool;

#[cfg(feature = "window")]
use winit::{
    dpi::{LogicalSize, LogicalPosition, PhysicalSize},
    platform::macos::WindowExtMacOS,
    window::Window,
    raw_window_handle::HasWindowHandle,
    event_loop::ActiveEventLoop,
};

thread_local!(
    static MTL_CONTEXT: RefCell<Option<MetalEngine>> = const { RefCell::new(None) };
    static MTL_STATUS: RefCell<Value> = const { RefCell::new(Value::Null) };
);

pub struct MetalEngine {
    context: DirectContext,
}

impl MetalEngine {
    pub fn supported() -> bool {
        MTL_CONTEXT.with_borrow_mut(|local_ctx| {
            if local_ctx.is_none(){
                *local_ctx = {
                    let (device, direct_context) = Device::system_default().map(|device| {
                        let command_queue = device.new_command_queue();
                        let backend_context = unsafe {
                            mtl::BackendContext::new(
                                device.as_ptr() as mtl::Handle,
                                command_queue.as_ptr() as mtl::Handle,
                            )
                        };
                        let direct_context = direct_contexts::make_metal(&backend_context, None)
                            .map(|context| MetalEngine{context});
                        (Some(device), direct_context)
                    }).unwrap_or((None, None));

                    Self::set_status(match device {
                        Some(device) => json!({
                            "renderer": "GPU",
                            "api": "Metal",
                            "device": format!("{} GPU ({})", match device.location(){
                                MTLDeviceLocation::BuiltIn => "Integrated GPU",
                                MTLDeviceLocation::Slot => "Discrete GPU",
                                MTLDeviceLocation::External => "External GPU",
                                _ => "Other"
                            }, device.name()),
                            "error": Value::Null,
                        }),
                        None => json!({
                            "renderer": "CPU",
                            "api": "Metal",
                            "device": "CPU-based renderer (Fallback)",
                            "error": "GPU initialization failed",

                        })
                    });

                    direct_context
                };
            }

            local_ctx.is_some()
        })
    }

    pub fn surface(image_info: &ImageInfo) -> Option<Surface> {
        MTL_CONTEXT.with_borrow(|local_ctx| {
            match local_ctx.is_some() || MetalEngine::supported(){
                true => surfaces::render_target(
                    &mut local_ctx.as_ref()?.context.clone(),
                    Budgeted::Yes,
                    image_info,
                    Some(4),
                    SurfaceOrigin::BottomLeft,
                    None,
                    true,
                    None
                ),
                false => None
            }
        })
    }

    pub fn set_status(msg: Value) {
        MTL_STATUS.with_borrow_mut(|status| *status = msg);
    }

    pub fn status() -> Value {
        MTL_STATUS.with_borrow(|err_cell| err_cell.clone() )
    }

}


pub struct MetalRenderer {
    layer: Arc<Mutex<MetalLayer>>,
    context: Arc<Mutex<DirectContext>>,
    queue: Arc<Mutex<CommandQueue>>,
}

unsafe impl Send for MetalRenderer {}

#[cfg(feature = "window")]
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

        let queue = device.new_command_queue();

        let backend = unsafe {
            mtl::BackendContext::new(
                device.as_ptr() as mtl::Handle,
                queue.as_ptr() as mtl::Handle,
            )
        };

        let context = direct_contexts::make_metal(&backend, None).unwrap();
        MetalRenderer {
            layer: Arc::new(Mutex::new(layer)),
            context: Arc::new(Mutex::new(context)),
            queue: Arc::new(Mutex::new(queue)),
        }
    }

    pub fn resize(&self, size: PhysicalSize<u32>) {
        self.layer
            .lock()
            .unwrap()
            .set_drawable_size(CGSize::new(size.width as f64, size.height as f64));
    }

    pub fn draw<F: FnOnce(&skia_safe::Canvas, LogicalSize<f32>)>(
        &mut self,
        window: &Window,
        f: F,
    ) -> Result<(), String> {
        let dpr = window.scale_factor();
        let size = window.inner_size();
        let layer = self.layer.lock().unwrap();
        let mut context = self.context.lock().unwrap();

        if let Some(drawable) = layer.next_drawable() {
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
                &mut context,
                &backend_render_target,
                SurfaceOrigin::TopLeft,
                ColorType::BGRA8888,
                None,
                None,
            ).unwrap();

            let canvas = surface.canvas();
            canvas.reset_matrix();
            canvas.scale((dpr as f32, dpr as f32));
            f(canvas, LogicalSize::from_physical(size, dpr));
            context.flush_and_submit();
            drop(surface);

            let queue = self.queue.lock().unwrap();
            let command_buffer = queue.new_command_buffer();
            command_buffer.present_drawable(drawable);
            command_buffer.commit();
            Ok(())
        }else{
            Err("Could not allocate frame buffer".to_string())
        }

    }
}