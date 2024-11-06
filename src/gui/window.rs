use std::sync::Arc;
use skia_safe::{Matrix, Color, Paint};
use serde::{Serialize, Deserialize};
use winit::{
    dpi::{LogicalSize, LogicalPosition, PhysicalSize},
    event_loop::{ActiveEventLoop, EventLoopProxy},
    window::{Window as WinitWindow, CursorIcon, Fullscreen},
};
#[cfg(target_os = "macos" )]
use winit::platform::macos::WindowExtMacOS;

use crate::utils::css_to_color;
use crate::gpu::{Renderer, runloop};
use crate::context::page::Page;
use super::event::CanvasEvent;

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WindowSpec {
    pub id: String,
    pub left: Option<f32>,
    pub top: Option<f32>,
    pub title: String,
    pub visible: bool,
    pub resizable: bool,
    pub fullscreen: bool,
    pub background: String,
    pub page: u32,
    pub width: f32,
    pub height: f32,
    #[serde(with = "Cursor")]
    pub cursor: CursorIcon,
    pub cursor_hidden: bool,
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
pub struct Window {
    pub handle: Arc<WinitWindow>,
    proxy: EventLoopProxy<CanvasEvent>,
    renderer: Renderer,
    fit: Fit,
    background: Color,
    page: Page,
    suspended: bool,
}

impl Window {
    pub fn new(event_loop:&ActiveEventLoop, proxy:EventLoopProxy<CanvasEvent>, spec: &mut WindowSpec, page: &Page) -> Self {
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
            .with_resizable(spec.resizable);
        let handle = Arc::new(event_loop.create_window(window_attributes).unwrap());

        if let (Some(left), Some(top)) = (spec.left, spec.top){
            handle.set_outer_position(LogicalPosition::new(left, top));
        }

        let renderer = Renderer::for_window(&event_loop, handle.clone());

        Self{ handle, proxy, renderer, page:page.clone(), fit:spec.fit, suspended:false, background }
    }

    pub fn resize(&mut self, size: PhysicalSize<u32>){
        if let Some(monitor) = self.handle.current_monitor(){
            self.renderer.resize(size);
            self.reposition_ime(size);

            let id = self.handle.id();
            self.proxy.send_event(CanvasEvent::Transform(id, self.fitting_matrix().invert() )).ok();
            self.proxy.send_event(CanvasEvent::InFullscreen(id, monitor.size() == size )).ok();
        }
    }

    pub fn reposition_ime(&mut self, size:PhysicalSize<u32>){
        // place the input region in the bottom left corner so the UI doesn't cover the window
        let dpr = self.handle.scale_factor();
        let window_height = size.to_logical::<u32>(dpr).height;
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

        let sf = match self.fit{
            Fit::Cover => fit_x.max(fit_y),
            Fit::ScaleDown => fit_x.min(fit_y).min(1.0),
            Fit::Contain => fit_x.min(fit_y),
            Fit::ContainX => fit_x,
            Fit::ContainY => fit_y,
            _ => 1.0
        };

        let (x_scale, y_scale) = match self.fit{
            Fit::Fill => (fit_x, fit_y),
            _ => (sf, sf)
        };

        let (x_shift, y_shift) = match self.fit{
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
        runloop(|| {
            let paint = Paint::default();
            let matrix = self.fitting_matrix();
            let (clip, _) = matrix.map_rect(self.page.bounds);

            self.renderer.draw(&self.handle, |canvas, _size| {
                canvas.clear(self.background);
                canvas.clip_rect(clip, None, Some(true));
                canvas.draw_picture(self.page.get_picture(None).unwrap(), Some(&matrix), Some(&paint));
            }).unwrap();
        })
    }

    pub fn handle_event(&mut self, event:CanvasEvent){
        runloop(|| {
            match event {
                CanvasEvent::Page(page) => {
                    self.page = page;
                    self.handle.request_redraw();
                }
                CanvasEvent::Visible(flag) => {
                    self.handle.set_visible(flag);
                }
                CanvasEvent::Resizable(flag) => {
                    self.handle.set_resizable(flag);
                }
                CanvasEvent::Title(title) => {
                    self.handle.set_title(&title);
                }
                CanvasEvent::Cursor(icon) => {
                    if let Some(icon) = icon{
                        self.handle.set_cursor(icon);
                    }
                    self.handle.set_cursor_visible(icon.is_some());
                }
                CanvasEvent::Fit(mode) => {
                    self.fit = mode;
                }
                CanvasEvent::Background(color) => {
                    self.background = color;
                }
                CanvasEvent::Size(size) => {
                    let size:PhysicalSize<u32> = size.to_physical(self.handle.scale_factor());
                    if let Some(to_size) = self.handle.request_inner_size(size){
                        self.resize(to_size);
                    }
                }
                CanvasEvent::Position(loc) => {
                    self.handle.set_outer_position(loc);
                }
                CanvasEvent::Fullscreen(to_fullscreen) => {
                    match to_fullscreen{
                        true => self.handle.set_fullscreen( Some(Fullscreen::Borderless(None)) ),
                        false => self.handle.set_fullscreen( None )
                    }
                }
                CanvasEvent::WindowResized(size) => {
                    self.resize(size);
                }
                CanvasEvent::RedrawingSuspended(suspended) => {
                    self.suspended = suspended;
                    if !suspended{
                        self.redraw();
                    }
                }
                CanvasEvent::RedrawRequested => {
                    if !self.suspended{
                        self.redraw()
                    }
                }

                _ => {}
            }
        })

    }
}

