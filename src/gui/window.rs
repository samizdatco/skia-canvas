use std::{str::FromStr, sync::Arc, time::{Instant, Duration}};
use skia_safe::{Matrix, Color};
use serde::{Serialize, Deserialize};
use winit::{
    dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize},
    window::{CursorIcon, Fullscreen, Window as WinitWindow, WindowId},
    event_loop::ActiveEventLoop,
};

use crate::utils::css_to_color;
use crate::gpu::Renderer;
use crate::context::page::Page;
use super::event::Sieve;

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WindowSpec {
    pub id: u32,
    pub left: Option<f32>,
    pub top: Option<f32>,
    pub title: String,
    pub visible: bool,
    pub resizable: bool,
    pub borderless: bool,
    pub fullscreen: bool,
    pub background: String,
    pub page: u32,
    pub width: f32,
    pub height: f32,
    pub cursor: String,
    pub fit: Fit,
}

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Fit{
  None, ContainX, ContainY, Contain, Cover, Fill, ScaleDown, Resize
}

#[non_exhaustive]
#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", remote = "CursorIcon" )]
pub enum Cursor {
    Alias, AllScroll, Cell, ColResize, ContextMenu, Copy, Crosshair, Default, EResize,
    EwResize, Grab, Grabbing, Help, Move, NeResize, NeswResize, NoDrop, NotAllowed,
    NResize, NsResize, NwResize, NwseResize, Pointer, Progress, RowResize, SeResize,
    SResize, SwResize, Text, VerticalText, Wait, WResize, ZoomIn, ZoomOut,
}

// timeout for triggering a full vector re-render after the last resize event
static RESIZE_CLEANUP_INTERVAL:Duration = Duration::from_millis(100);

pub struct Window {
    pub handle: Arc<WinitWindow>,
    pub spec: WindowSpec,
    pub sieve: Sieve,
    renderer: Renderer,
    background: Color,
    page: Page,
    suspended: bool,
    resized_at: Option<Instant>,
}

impl Window {
    pub fn new(event_loop:&ActiveEventLoop, mut spec:WindowSpec, page:&Page) -> Self {
        let size:LogicalSize<i32> = LogicalSize::new(spec.width as i32, spec.height as i32);
        let background = match css_to_color(&spec.background){
            Some(color) => color,
            None => {
                spec.background = "rgba(16,16,16,0.85)".to_string();
                css_to_color(&spec.background).unwrap()
            }
        };

        let window_attributes = WinitWindow::default_attributes()
            .with_fullscreen(if spec.fullscreen{ Some(Fullscreen::Borderless(None)) }else{ None })
            .with_inner_size(size)
            .with_transparent(background.a() < 255)
            .with_title(spec.title.clone())
            .with_visible(false)
            .with_resizable(spec.resizable)
            .with_decorations(!spec.borderless);

        let handle = Arc::new(event_loop.create_window(window_attributes).unwrap());
        let renderer = Renderer::for_window(&event_loop, handle.clone());
        let sieve = Sieve::new(handle.scale_factor());

        let cursor_icon = CursorIcon::from_str(&spec.cursor).ok();
        handle.set_cursor(cursor_icon.unwrap_or_default());
        handle.set_cursor_visible(cursor_icon.is_some());

        if let (Some(left), Some(top)) = (spec.left, spec.top){
            handle.set_outer_position(LogicalPosition::new(left, top));
        }

        Self{ spec, handle, sieve, renderer, page:page.clone(), suspended:false, resized_at:None, background}
    }

    pub fn id(&self) -> WindowId {
        self.handle.id()
    }

    pub fn resize(&mut self, size: PhysicalSize<u32>){
        self.resized_at = Some(Instant::now());
        self.renderer.resize(size);
        self.reposition_ime(size);
        self.update_fit();

        let LogicalSize{width, height} = self.handle.inner_size().to_logical::<f32>(self.handle.scale_factor());
        let is_fullscreen = self.handle.fullscreen().is_some()
            && width >= self.spec.width
            && height >= self.spec.height;

        self.spec = WindowSpec{width, height, ..self.spec.clone()};
        if self.spec.fullscreen != is_fullscreen{
            self.sieve.go_fullscreen(is_fullscreen);
            self.spec.fullscreen = is_fullscreen;
        }

        #[cfg(feature = "vulkan")]
        self.handle.request_redraw();
    }

    pub fn reposition(&mut self, loc:LogicalPosition<i32>){
        self.spec.left = Some(loc.x as _);
        self.spec.top = Some(loc.y as _);
    }

    pub fn update_fit(&mut self){
        if let Some(fit) = self.fitting_matrix().invert(){
            self.sieve.use_transform(fit);
        }
    }

    pub fn reposition_ime(&mut self, size:PhysicalSize<u32>){
        // place the input region in the bottom left corner so the UI doesn't cover the window
        let dpr = self.handle.scale_factor();
        let window_height = size.to_logical::<i32>(dpr).height;
        self.handle.set_ime_allowed(true);
        self.handle.set_ime_cursor_area(
            LogicalPosition::new(15, window_height-20), LogicalSize::new(100, 15)
        );
    }

    pub fn fitting_matrix(&self) -> Matrix {
        let dpr = self.handle.scale_factor();
        let size = self.handle.inner_size().to_logical::<f32>(dpr);
        let dims = self.page.bounds.size();
        let fit_x = size.width / dims.width;
        let fit_y = size.height / dims.height;

        let sf = match self.spec.fit{
            Fit::Cover => fit_x.max(fit_y),
            Fit::ScaleDown => fit_x.min(fit_y).min(1.0),
            Fit::Contain => fit_x.min(fit_y),
            Fit::ContainX => fit_x,
            Fit::ContainY => fit_y,
            _ => 1.0
        };

        let (x_scale, y_scale) = match self.spec.fit{
            Fit::Fill => (fit_x, fit_y),
            _ => (sf, sf)
        };

        let (x_shift, y_shift) = match self.spec.fit{
            Fit::Resize => (0.0, 0.0),
            _ => ( (size.width - dims.width * x_scale) / 2.0,
                   (size.height - dims.height * y_scale) / 2.0 )
        };

        let mut matrix = Matrix::new_identity();
        matrix.set_scale_translate(
            (x_scale, y_scale),
            (x_shift, y_shift)
        );
        matrix
    }

    pub fn redraw(&mut self){
        if !self.suspended{
            self.renderer.draw(self.page.clone(), self.fitting_matrix(), self.background);
        }
    }

    pub fn set_page(&mut self, page:Page){
        if self.page != page{
            self.handle.request_redraw();
        }
        self.page = page;
    }

    pub fn set_visible(&mut self, flag:bool){
        self.handle.set_visible(flag);
    }

    pub fn set_resizable(&mut self, flag:bool){
        self.handle.set_resizable(flag);
    }

    pub fn set_borderless(&mut self, flag:bool){
        self.handle.set_decorations(!flag);
    }

    pub fn set_title(&mut self, title:&str){
        self.handle.set_title(title);
    }

    pub fn set_cursor(&mut self, icon:&str){
        let cursor_icon = CursorIcon::from_str(icon).ok();
        self.handle.set_cursor(cursor_icon.unwrap_or_default());
        self.handle.set_cursor_visible(cursor_icon.is_some());
    }

    pub fn set_fit(&mut self, mode:Fit){
        self.spec.fit = mode;
    }

    pub fn set_background(&mut self, color:Color){
        if self.background != color{
            self.background = color;
            self.handle.request_redraw();
        }
    }

    pub fn set_size(&mut self, size:LogicalSize<u32>){
        let size:PhysicalSize<u32> = size.to_physical(self.handle.scale_factor());
        if let Some(to_size) = self.handle.request_inner_size(size){
            self.resize(to_size);
        }
    }

    pub fn set_position(&mut self, loc:LogicalPosition<i32>){
        self.handle.set_outer_position(loc);
        self.reposition(loc);
    }

    pub fn set_fullscreen(&mut self, to_fullscreen:bool){
        match to_fullscreen{
            true => self.handle.set_fullscreen( Some(Fullscreen::Borderless(None)) ),
            false => self.handle.set_fullscreen( None )
        }
    }

    pub fn did_move(&mut self, size:PhysicalPosition<i32>){
        self.reposition(size.to_logical(self.handle.scale_factor()));
    }

    pub fn did_resize(&mut self, size:PhysicalSize<u32>){
        self.resize(size);
    }

    pub fn redraw_if_resized(&mut self){
        if let Some(resize) = self.resized_at{
            if resize.elapsed() > RESIZE_CLEANUP_INTERVAL{
                self.resized_at = None;
                self.handle.request_redraw();
            }
        }
    }

    pub fn set_redrawing_suspended(&mut self, suspended:bool){
        self.suspended = suspended;
        if !suspended{
            self.handle.request_redraw();
        }
    }
}

