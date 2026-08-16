use std::sync::Arc;
use std::time::Duration;
use objc2::{rc::{autoreleasepool, Retained}, runtime::ProtocolObject};
use objc2_core_foundation::CGSize;
use objc2_metal::{
    MTLCommandBuffer, MTLCommandQueue, MTLCreateSystemDefaultDevice, MTLPixelFormat,
};
use objc2_quartz_core::{kCAGravityBottomLeft, kCAGravityTopLeft, CAMetalDrawable, CAMetalLayer};
use objc2_core_graphics::{CGColorSpace, kCGColorSpaceDisplayP3};
use skia_safe::{
    scalar, ColorType, ColorSpace, Size, Matrix, Color4f, Paint, BlendMode, SurfaceProps,
    canvas::SrcRectConstraint,
};
use skia_safe::gpu::{ mtl, surfaces, backend_render_targets, SurfaceOrigin, DirectContext };
use raw_window_metal::Layer;
use winit::{
    dpi::PhysicalSize,
    window::Window,
    raw_window_handle::{RawWindowHandle, HasWindowHandle},
    event_loop::ActiveEventLoop,
};

use crate::gfx::page::{Page, Replay};
use crate::gfx::RenderOutcome;
use crate::gfx::cache::Frame;
use crate::bridge::to_color_space;
use super::make_direct_context;

pub struct MetalRenderer {
    window: Arc<Window>,
    backend: MetalBackend,
    layer: Retained<CAMetalLayer>,
    frame: Frame,
    last_page_id: usize, // previous frame's page id — render-loop history, not cache state
}

impl MetalRenderer{
    pub fn for_window(_event_loop: &ActiveEventLoop, window:Arc<Window>, is_transparent:bool) -> Self {
        let device = MTLCreateSystemDefaultDevice().expect("Metal device not found");

        let raw_window = window
            .window_handle()
            .expect("Failed to retrieve a window handle")
            .as_raw();

        let raw_layer = match raw_window {
            RawWindowHandle::AppKit(handle) => unsafe { Layer::from_ns_view(handle.ns_view) },
            RawWindowHandle::UiKit(handle) => unsafe { Layer::from_ui_view(handle.ui_view) },
            _ => panic!("Unsupported window handle type"),
        };

        // raw-window-metal hands back a +1 retained CAMetalLayer pointer
        let layer: Retained<CAMetalLayer> = unsafe {
            Retained::from_raw(raw_layer.into_raw().as_ptr().cast())
                .expect("Failed to obtain a CAMetalLayer")
        };

        let gravity = unsafe {
            match layer.contentsAreFlipped() {
                true => kCAGravityBottomLeft,
                false => kCAGravityTopLeft,
            }
        };
        layer.setContentsGravity(gravity);
        layer.setOpaque(false);
        let (pixel_format, color_type) = match is_transparent {
            true  => (MTLPixelFormat::RGBA16Float,  ColorType::RGBAF16),
            false => (MTLPixelFormat::RGB10A2Unorm, ColorType::RGBA1010102),
        };

        unsafe{
            layer.setDevice(Some(&device));
            layer.setPixelFormat(pixel_format);
            // tag the layer as Display P3 so CoreAnimation color-manages our P3 pixels to whatever
            // gamut the window's display has (unchanged on a P3 panel, gamut-mapped down on sRGB)
            let p3 = CGColorSpace::with_name(Some(kCGColorSpaceDisplayP3));
            layer.setColorspace(p3.as_deref());
            layer.setPresentsWithTransaction(false);
            layer.setDisplaySyncEnabled(true);
            layer.setFramebufferOnly(false); // to enable blend modes

            let draw_size = window.inner_size();
            layer.setDrawableSize(CGSize::new(draw_size.width as f64, draw_size.height as f64));
        }

        let backend = MetalBackend::for_layer(&layer, color_type);
        let frame = Frame::default();

        Self{window, layer, backend, frame, last_page_id:0}
    }

    pub fn resize(&mut self, size: PhysicalSize<u32>) {
        let cg_size = CGSize::new(size.width as f64, size.height as f64);
        unsafe{ self.layer.setDrawableSize(cg_size) };
        self.frame.start_resizing();
    }

    pub fn draw(&mut self, page:Page, matrix:Matrix, props:SurfaceProps, matte:Color4f){
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
            page.playback_from(canvas, self.frame.depth(), Some(&matrix), Replay::Bitmap);
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
    queue: Retained<ProtocolObject<dyn MTLCommandQueue>>,
    color_type: ColorType,   // the layer's pixel format (RGBA1010102 / BGRA8888 / RGBAF16)
    color_space: ColorSpace, // the layer's colorspace tag (Display P3)
}

impl MetalBackend {
    pub fn for_layer(layer:&CAMetalLayer, color_type:ColorType) -> Self{
        let device = unsafe{ layer.device() }.expect("Metal: layer has no device");
        let (queue, skia_ctx) = make_direct_context(&device)
            .expect("Metal: Failed to create Skia direct context");
        Self { skia_ctx, queue, color_type, color_space:to_color_space("display-p3") }
    }

    fn render_to_layer<F>(&mut self, layer:&CAMetalLayer, window:&Window, sync:bool, snapshot:bool, props:&SurfaceProps, f:F) -> RenderOutcome
        where F:FnOnce(&skia_safe::Canvas)
    {
      autoreleasepool(|_|{
        // if layer pool is exhausted, just drop the frame and try again next tick
        let Some(drawable) = (unsafe{ layer.nextDrawable() }) else {
            return RenderOutcome::Skipped;
        };

        let drawable_size = {
            let size = unsafe{ layer.drawableSize() };
            Size::new(size.width as scalar, size.height as scalar)
        };

        let backend_render_target = unsafe {
            let texture = drawable.texture();
            let texture_info =
                mtl::TextureInfo::new(Retained::as_ptr(&texture) as mtl::Handle);
            backend_render_targets::make_mtl(
                (drawable_size.width as i32, drawable_size.height as i32),
                &texture_info,
            )
        };

        let mut surface = surfaces::wrap_backend_render_target(
            &mut self.skia_ctx,
            &backend_render_target,
            SurfaceOrigin::TopLeft,
            self.color_type,
            Some(self.color_space.clone()),
            Some(props),
        ).expect("MetalBackend: could not create render target");

        // pass the suface's canvas to the user-provided callback
        f(surface.canvas());

        self.skia_ctx.flush_and_submit();
        self.skia_ctx.perform_deferred_cleanup(Duration::from_secs(1), None);

        window.pre_present_notify();
        let command_buffer = self.queue.commandBuffer()
            .expect("MetalBackend: could not create command buffer");
        command_buffer.presentDrawable(ProtocolObject::from_ref(&*drawable));
        command_buffer.commit();

        // during resizes, ensure drawing is complete before returning
        if sync{ unsafe{ command_buffer.waitUntilCompleted() }; }

        // copy the frame contents (for the Frame cache) only when they'll be reused
        RenderOutcome::Rendered(snapshot.then(||
          surface.image_snapshot_with_bounds(surface.image_info().bounds())).flatten()
        )
      })
    }

}
