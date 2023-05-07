#![allow(unused_imports)]
#![allow(dead_code)]

use std::cell::RefCell;
use cocoa::{appkit::NSView, base::id as cocoa_id};
use core_graphics_types::geometry::CGSize;
use foreign_types::{ForeignType, ForeignTypeRef};
use metal::{CommandQueue, Device, MTLPixelFormat, MetalLayer};
use objc::runtime::YES;
use std::sync::{Arc, Mutex};
use skia_safe::{
    gpu::{mtl, BackendRenderTarget, DirectContext, SurfaceOrigin},
    scalar, Budgeted, ImageInfo, ColorType, Size, Surface,
};
pub use objc::rc::autoreleasepool;

#[cfg(feature = "window")]
use winit::{
    dpi::{LogicalSize, LogicalPosition, PhysicalSize},
    platform::macos::WindowExtMacOS,
    window::{Window, WindowBuilder},
};

thread_local!(static MTL_CONTEXT: RefCell<Option<MetalEngine>> = RefCell::new(None));

pub struct MetalEngine {
    context: DirectContext,
}

impl MetalEngine {
    fn init() {
        MTL_CONTEXT.with(|cell| {
            let mut local_ctx = cell.borrow_mut();
            if local_ctx.is_none(){
                if let Some(ctx) = MetalEngine::new() {
                    local_ctx.replace(ctx);
                }
            }
        })
    }

    pub fn supported() -> bool {
        Self::init();
        MTL_CONTEXT.with(|cell| cell.borrow().is_some() )
    }

    pub fn new() -> Option<Self> {
      let device = Device::system_default()?;
      let command_queue = device.new_command_queue();
      let backend_context = unsafe {
          mtl::BackendContext::new(
              device.as_ptr() as mtl::Handle,
              command_queue.as_ptr() as mtl::Handle,
              std::ptr::null(),
          )
      };
      if let Some(context) = DirectContext::new_metal(&backend_context, None){
          Some(MetalEngine{context})
      }else{
          None
      }

    }

    pub fn surface(image_info: &ImageInfo) -> Option<Surface> {
        Self::init();
        MTL_CONTEXT.with(|cell| {
            let local_ctx = cell.borrow();
            let mut context = local_ctx.as_ref().unwrap().context.clone();

            Surface::new_render_target(
                &mut context,
                Budgeted::Yes,
                image_info,
                Some(4),
                SurfaceOrigin::BottomLeft,
                None,
                true,
            )
        })
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
    pub fn for_window(window: &Window) -> Self {
        let device = Device::system_default().expect("no device found");

        let layer = {
            let draw_size = window.inner_size();
            let layer = MetalLayer::new();
            layer.set_device(&device);
            layer.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
            layer.set_presents_with_transaction(false);
            layer.set_opaque(false);

            unsafe {
                let view = window.ns_view() as cocoa_id;
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
                std::ptr::null(),
            )
        };

        let context = DirectContext::new_metal(&backend, None).unwrap();
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

    pub fn draw<F: FnOnce(&mut skia_safe::Canvas, LogicalSize<f32>)>(
        &mut self,
        window: &Window,
        f: F,
    ) -> Result<(), String> {
        let dpr = window.scale_factor();
        let size = window.inner_size();
        let layer = self.layer.lock().unwrap();

        if let Some(drawable) = layer.next_drawable() {
            let drawable_size = {
                let size = layer.drawable_size();
                Size::new(size.width as scalar, size.height as scalar)
            };

            let mut surface = unsafe {
                let texture_info =
                    mtl::TextureInfo::new(drawable.texture().as_ptr() as mtl::Handle);

                let backend_render_target = BackendRenderTarget::new_metal(
                    (drawable_size.width as i32, drawable_size.height as i32),
                    1,
                    &texture_info,
                );

                Surface::from_backend_render_target(
                    &mut self.context.lock().unwrap(),
                    &backend_render_target,
                    SurfaceOrigin::TopLeft,
                    ColorType::BGRA8888,
                    None,
                    None,
                )
                .unwrap()
            };

            let canvas = surface.canvas();
            canvas.reset_matrix();
            canvas.scale((dpr as f32, dpr as f32));
            f(canvas, LogicalSize::from_physical(size, dpr));
            surface.flush_and_submit();
            drop(surface);

            let queue = self.queue.lock().unwrap();
            let command_buffer = queue.new_command_buffer();
            command_buffer.present_drawable(drawable);
            command_buffer.commit();
            Ok(())
        }else{
            Err(format!("Could not allocate frame buffer"))
        }

    }
}