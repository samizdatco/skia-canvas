use skia_safe::{Rect, Matrix, Color4f, Paint, BlendMode, Image as SkImage,
                Canvas as SkCanvas, canvas::SrcRectConstraint};
use crate::gfx::RenderOutcome;
use crate::gfx::cache::Cache;
use crate::gfx::page::{Page, PageVersion, Replay};

// settings decided up front before drawing the frame
pub struct FramePlan{
    pub take_snapshot: bool,
    clip: Rect,
    dpr: f32,
}

// result of the last drawing pass
#[derive(Debug, PartialEq, Clone, Copy)]
enum FrameState{
    Clean,
    Dirty,
    Resizing
}

//
// Bitmap of a window's last render (so the next pass can layer onto it). Owned by its Renderer
//

pub struct Frame {
    image: Option<SkImage>,
    content: Rect, // the region occupied by the canvas (in device pixels)
    version: PageVersion, // the page state (id, bounds epoch, layer count) captured by the snapshot
    matte: Color4f, // the window background color
    dpr: f32, // current monitor's density
    state: FrameState, // whether the window needs redisplay (and why)
    last_page_id: usize, // page id from the last render pass
}

impl Default for Frame{
    fn default() -> Self {
        Self{image:None, content:Rect::new_empty(), version:PageVersion::default(), dpr:0.0, matte:Color4f::new(0.0, 0.0, 0.0, 0.0), state:FrameState::Clean, last_page_id:0}
    }
}

impl Frame{
    pub fn begin(&self, page:&Page, matrix:&Matrix, matte:Color4f, dpr:f32) -> FramePlan{
        // decide on the viewport geometry and whether to snapshot before receiving the surface
        let (clip, _) = matrix.map_rect(page.bounds);
        FramePlan{ clip, dpr, take_snapshot: self.wants_snapshot(page, matte, dpr) }
    }

    pub fn draw(&mut self, canvas:&SkCanvas, page:&Page, matrix:&Matrix, matte:Color4f, plan:&FramePlan, cache:Cache){
        canvas.clear(matte); // fill the full window bounds (including letterboxing)
        if self.state == FrameState::Dirty || !self.is_current(page, matte, plan.dpr){
            *self = Self::default(); // start from zero if the saved image no longer matches
        }

        // if the snapshot (which includes the matte) is still valid, draw it to the content area
        if let Some(image) = &self.image{
            let (dst, _) = Matrix::scale((plan.dpr, plan.dpr)).map_rect(plan.clip);
            let mut paint = Paint::default();
            paint.set_blend_mode(BlendMode::Src); // overwrite so the matte isn't duplicated
            canvas.draw_image_rect(image, Some((&self.content, SrcRectConstraint::Strict)), dst, &paint);
        }

        // replay just the layers that are newer than the snapshot
        canvas.scale((plan.dpr, plan.dpr))
              .clip_rect(plan.clip, None, Some(true));
        page.playback_from(canvas, self.version.depth, Some(matrix), Replay::Raster(cache));
    }

    pub fn commit(&mut self, outcome:RenderOutcome, page:&Page, matte:Color4f, plan:&FramePlan){
        // save the updated canvas contents for the next render pass
        let RenderOutcome::Rendered(image) = outcome else { return };
        self.last_page_id = page.id;

        if self.state == FrameState::Resizing{
            // mark the framebuffer as needing a full redraw rather than caching a mid-resize frame
            self.state = FrameState::Dirty;
        }else if let Some(image) = image{
            // replace the snapshot if new drawing has been layered on top
            let (content, _) = Matrix::scale((plan.dpr, plan.dpr)).map_rect(plan.clip);
            *self = Self{image:Some(image), version:page.version(), matte, dpr:plan.dpr, content,
                         state:FrameState::Clean, last_page_id:page.id};
        }
    }

    fn is_current(&self, page:&Page, matte:Color4f, dpr:f32) -> bool{
        // whether the bitmap is still valid for the page about to be drawn
        self.version.extends(&page.version()) && self.matte == matte && self.dpr == dpr
    }

    fn wants_snapshot(&self, page:&Page, matte:Color4f, dpr:f32) -> bool{
        // decide (before rendering) whether the frame's snapshot would ever be drawn: skip the
        // GPU→GPU copy when it would just be discarded
        if self.state == FrameState::Resizing{
            return false // commit() drops the image during resizes
        }

        let cache_is_current = self.state == FrameState::Clean &&
            self.image.is_some() &&
            self.is_current(page, matte, dpr);

        match cache_is_current{
            // re-snapshot when new layers extend the cached content
            true => page.depth() > self.version.depth,
            // otherwise snapshot only for pages that persist across frames (an id that churns
            // frame-to-frame means full-page redraws that invalidate before reuse)
            false => page.id == self.last_page_id
        }
    }

    pub fn start_resizing(&mut self){
        // a resizing invalidates the snapshot until the next full redraw completes
        self.state = FrameState::Resizing;
    }
}
