#![allow(unexpected_cfgs)]
use std::sync::Arc;
use std::time::Duration;
use metal::{
    foreign_types::{ForeignType, ForeignTypeRef},
    CommandQueue, Device, MTLPixelFormat, MetalLayer,
};
use skia_safe::{
    scalar, ColorType, Size, Matrix, Color, Paint, BlendMode, SurfaceProps,
    canvas::SrcRectConstraint,
};
use skia_safe::gpu::{ mtl, surfaces, backend_render_targets, SurfaceOrigin, DirectContext };
use objc::rc::autoreleasepool;
use raw_window_metal::Layer;
use core_graphics_types::geometry::CGSize;
use objc::{msg_send, sel, sel_impl, runtime::{self, Object}};
use winit::{
    dpi::PhysicalSize,
    window::Window,
    raw_window_handle::{RawWindowHandle, HasWindowHandle},
    event_loop::ActiveEventLoop,
};

use crate::context::page::Page;
use crate::gfx::RenderOutcome;
use crate::gfx::cache::Frame;
use super::make_direct_context;

#[allow(non_upper_case_globals)]
#[link(name = "QuartzCore", kind = "framework")]
unsafe extern "C" {
    static kCAGravityTopLeft: *mut Object;
    static kCAGravityBottomLeft: *mut Object;
}

pub struct MetalRenderer {
    window: Arc<Window>,
    backend: MetalBackend,
    layer: MetalLayer,
    frame: Frame,
    last_page_id: usize, // previous frame's page id — render-loop history, not cache state
}

impl MetalRenderer{
    pub fn for_window(_event_loop: &ActiveEventLoop, window:Arc<Window>) -> Self {
        let device = Device::system_default().expect("Metal device not found");

        let raw_window = window
            .window_handle()
            .expect("Failed to retrieve a window handle")
            .as_raw();

        let raw_layer = match raw_window {
            RawWindowHandle::AppKit(handle) => unsafe { Layer::from_ns_view(handle.ns_view) },
            RawWindowHandle::UiKit(handle) => unsafe { Layer::from_ui_view(handle.ui_view) },
            _ => panic!("Unsupported window handle type"),
        };

        let layer = unsafe{
            let mtl_layer = MetalLayer::from_ptr(raw_layer.into_raw().as_ptr().cast());
            let gravity = match msg_send![mtl_layer.as_ptr(), contentsAreFlipped] {
                runtime::YES => kCAGravityBottomLeft,
                _ => kCAGravityTopLeft,
            };
            let _: () = msg_send![mtl_layer.as_ptr(), setContentsGravity: gravity];
            mtl_layer
        };
        layer.set_device(&device);
        layer.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
        layer.set_presents_with_transaction(false);
        layer.set_display_sync_enabled(true);
        layer.set_opaque(false);
        layer.set_framebuffer_only(false); // to enable blend modes

        let draw_size = window.inner_size();
        layer.set_drawable_size(CGSize::new(draw_size.width as f64, draw_size.height as f64));

        let backend = MetalBackend::for_layer(&layer);
        let frame = Frame::default();

        Self{window, layer, backend, frame, last_page_id:0}
    }

    pub fn resize(&mut self, size: PhysicalSize<u32>) {
        let cg_size = CGSize::new(size.width as f64, size.height as f64);
        self.layer.set_drawable_size(cg_size);
        self.frame.start_resizing();
    }

    pub fn draw(&mut self, page:Page, matrix:Matrix, props:SurfaceProps, matte:Color){
        let (clip, _) = matrix.map_rect(page.bounds);
        let dpr = self.window.scale_factor() as f32;
        let sync = self.frame.is_resizing();
        let take_snapshot = self.frame.wants_snapshot(&page, matte, dpr, self.last_page_id);

        let outcome = self.backend.render_to_layer(&self.layer, &self.window, sync, take_snapshot, &props, |canvas| {
            // fill the full surface (including any letterboxing) with the window’s background
            // color, then lay the raster cache (if any) over the content area
            canvas.clear(matte);
            if let Some((image, src, dst)) = self.frame.validate(&page, matte, dpr, clip){
                let mut paint = Paint::default();
                paint.set_blend_mode(BlendMode::Src); // cached frame already includes the matte
                canvas.draw_image_rect(image, Some((src, SrcRectConstraint::Strict)), dst, &paint);
            }

            // draw newly added vector layers
            canvas.scale((dpr, dpr))
                .clip_rect(clip, None, Some(true));
            for pict in page.layers.iter().skip(self.frame.depth()){
                canvas.draw_picture(pict, Some(&matrix), None);
            }
        });

        // cache frame contents for use as background of next render pass
        if let RenderOutcome::Rendered(image) = outcome{
            self.frame.update(image, &page, matte, dpr, clip);
            self.last_page_id = page.id;
        }
    }
}

pub struct MetalBackend {
    skia_ctx: DirectContext,
    queue: CommandQueue,
}

impl MetalBackend {
    pub fn for_layer(layer:&MetalLayer) -> Self{
        let (queue, skia_ctx) = make_direct_context(&layer.device())
            .expect("Metal: Failed to create Skia direct context");
        Self { skia_ctx, queue }
    }

    fn render_to_layer<F>(&mut self, layer:&MetalLayer, window:&Window, sync:bool, snapshot:bool, props:&SurfaceProps, f:F) -> RenderOutcome
        where F:FnOnce(&skia_safe::Canvas)
    {
      autoreleasepool(||{
        // if layer pool is exhausted, just drop the frame and try again next tick
        let Some(drawable) = layer.next_drawable() else {
            return RenderOutcome::Skipped;
        };

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
            Some(props),
        ).expect("MetalBackend: could not create render target");

        // pass the suface's canvas to the user-provided callback
        f(surface.canvas());

        self.skia_ctx.flush_and_submit();
        self.skia_ctx.perform_deferred_cleanup(Duration::from_secs(1), None);

        window.pre_present_notify();
        let command_buffer = self.queue.new_command_buffer();
        command_buffer.present_drawable(drawable);
        command_buffer.commit();

        // during resizes, ensure drawing is complete before returning
        if sync{ command_buffer.wait_until_completed(); }

        // copy the frame contents (for the Frame cache) only when they'll be reused
        RenderOutcome::Rendered(snapshot.then(||
          surface.image_snapshot_with_bounds(surface.image_info().bounds())).flatten()
        )
      })
    }

}
