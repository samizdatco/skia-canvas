use std::thread;
use neon::prelude::*;
use serde_json::json;
use skia_safe::{Matrix, Point, Color, Paint};
use crossbeam::channel::{self, Sender, Receiver};
use serde::{Serialize, Deserialize};
use winit::{
    dpi::{LogicalSize, LogicalPosition, PhysicalSize, PhysicalPosition},
    event::{Event, WindowEvent},
    event_loop::{EventLoopWindowTarget, EventLoopProxy},
    window::{Window as WinitWindow, WindowBuilder, WindowId, CursorIcon, Fullscreen},
};
#[cfg(target_os = "macos" )]
use winit::platform::macos::WindowExtMacOS;

use crate::utils::css_to_color;
use crate::gpu::{Renderer, runloop};
use crate::context::page::Page;
use super::event::{CanvasEvent, Sieve};

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct WindowSpec {
    pub id: String,
    pub left: i32,
    pub top: i32,
    title: String,
    visible: bool,
    fullscreen: bool,
    background: String,
    page: u32,
    width: f32,
    height: f32,
    #[serde(with = "Cursor")]
    cursor: CursorIcon,
    fit: Fit,
}

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Fit{
  None, ContainX, ContainY, Contain, Cover, Fill, ScaleDown
}

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", remote = "CursorIcon" )]
pub enum Cursor {
    Default, Crosshair, Hand, Arrow, Move, Text, Wait, Help, Progress, NotAllowed, ContextMenu,
    Cell, VerticalText, Alias, Copy, NoDrop, Grab, Grabbing, AllScroll, ZoomIn, ZoomOut, EResize,
    NResize, NeResize, NwResize, SResize, SeResize, SwResize, WResize, EwResize, NsResize, NeswResize,
    NwseResize, ColResize, RowResize
}

pub struct Window {
    pub handle: WinitWindow,
    pub proxy: EventLoopProxy<CanvasEvent>,
    pub renderer: Renderer,
    pub fit: Fit,
    pub background: Color,
    pub page: Page
}

impl Finalize for Window {}

impl Window {
    pub fn new(event_loop:&EventLoopWindowTarget<CanvasEvent>, proxy:EventLoopProxy<CanvasEvent>, spec: &WindowSpec, page: &Page) -> Self {
        let size:LogicalSize<i32> = LogicalSize::new(spec.width as i32, spec.height as i32);
        // let loc:LogicalPosition<i32> = LogicalPosition::new(500+spec.x, 300+spec.y);
        let handle = WindowBuilder::new()
            .with_inner_size(size)
            // .with_position(loc)
            .with_transparent(true)
            .with_title(spec.title.clone())
            .build(&event_loop)
            .unwrap();
        let renderer = Renderer::for_window(&handle);
        let background = css_to_color(&spec.background).unwrap_or(Color::BLACK);

        Self{ handle, proxy, renderer, page:page.clone(), fit:spec.fit, background }
    }

    pub fn resize(&self, size: PhysicalSize<u32>){
        self.renderer.resize(size);
        self.proxy.send_event(CanvasEvent::Transform(
            self.handle.id(),
            self.fitting_matrix().invert()
        )).ok();
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

        let x_shift = (size.width - dims.width * x_scale) / 2.0;
        let y_shift = (size.height - dims.height * y_scale) / 2.0;

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
            let (clip, _) = matrix.map_rect(&self.page.bounds);

            self.renderer.draw(&self.handle, |canvas, _size| {
                canvas.clear(self.background);
                canvas.clip_rect(&clip, None, Some(true));
                canvas.draw_picture(self.page.get_picture(None).unwrap(), Some(&matrix), Some(&paint));
            }).unwrap();
        })
    }

    pub fn handle_event(&mut self, event:Event<CanvasEvent>){
        runloop(|| {
            match event {
                Event::UserEvent(canvas_event) => {
                    match canvas_event{
                        CanvasEvent::Page(page) => {
                            self.page = page;
                            self.handle.request_redraw();
                        }
                        CanvasEvent::Visible(flag) => {
                            self.handle.set_visible(flag);
                        }
                        CanvasEvent::Title(title) => {
                            self.handle.set_title(&title);
                        }
                        CanvasEvent::Cursor(icon) => {
                            self.handle.set_cursor_icon(icon);
                        }
                        CanvasEvent::Fit(mode) => {
                            self.fit = mode;
                        }
                        CanvasEvent::Background(color) => {
                            self.background = color;
                        }
                        CanvasEvent::Fullscreen(to_fullscreen) => {
                            match to_fullscreen{
                                true => self.handle.set_fullscreen( Some(Fullscreen::Borderless(None)) ),
                                false => self.handle.set_fullscreen( None )
                            }
                        }
                        _ => { println!("update {:?}", canvas_event); }
                    }
                }

                Event::WindowEvent { event, window_id } => match event {
                    WindowEvent::Resized(size) => {
                        self.resize(size);
                        let in_fullscreen = self.handle.fullscreen().is_some();
                        self.proxy.send_event(CanvasEvent::InFullscreen(window_id, in_fullscreen)).ok();
                        self.handle.request_redraw();
                    },
                    _ => {}
                }
                Event::RedrawRequested(_) => {
                    self.redraw()
                },
                _ => {}
            }
        })

    }
}

struct WindowRef { tx: Sender<Event<'static, CanvasEvent>>, id: WindowId, spec: WindowSpec, sieve:Sieve }
pub struct WindowManager {
    windows: Vec<WindowRef>,
    last: Option<PhysicalPosition<i32>>,
}

impl Default for WindowManager {
    fn default() -> Self {
        Self{ windows: vec![], last: None }
    }
}

impl WindowManager {

    pub fn add(&mut self, mut window:Window, mut spec:WindowSpec){
        let id = window.handle.id();
        let (tx, rx) = channel::bounded(50);
        let mut sieve = Sieve::new(window.handle.scale_factor());
        if let Some(fit) = window.fitting_matrix().invert(){
            sieve.use_transform(fit);
        }

        // cascade the windows based on the position of the most recently opened
        if let Ok(loc) = window.handle.outer_position(){
            if let Ok(inset) = window.handle.inner_position(){
                let delta = inset.y - loc.y;
                let corner = match self.last {
                    Some(last) => PhysicalPosition::new(last.x + delta, last.y + delta),
                    None => loc
                };
                self.last = Some(corner);
                window.handle.set_outer_position(corner);
            }
        }

        if let Ok(loc) = window.handle.outer_position(){
            spec.left = loc.x;
            spec.top = loc.y;
        }

        self.windows.push( WindowRef{ id, spec, tx, sieve } );

        thread::spawn(move || {
            while let Ok(event) = rx.recv() {
                window.handle_event(event);
            }
        });
    }

    pub fn remove(&mut self, window_id:&WindowId){
        self.windows.retain(|win| win.id != *window_id);
    }

    pub fn remove_by_token(&mut self, token:&str){
        self.windows.retain(|win| win.spec.id != token);
    }

    pub fn id_for(&self, token:&str) -> Option<WindowId> {
        self.windows.iter().find(|win| win.spec.id == *token).map(|win| win.id.clone())
    }

    pub fn send_event(&self, window_id:&WindowId, event:Event<CanvasEvent>){
        if let Some(tx) = self.windows.iter().find(|win| win.id == *window_id).map(|win| &win.tx){
            if let Some(event) = event.to_static() {
                tx.send(event).ok();
            }
        }
    }

    pub fn update_window(&mut self, spec:WindowSpec, page:Page){
        let mut updates:Vec<CanvasEvent> = vec![];

        if let Some(mut win) = self.windows.iter_mut().find(|win| win.spec.id == spec.id){
            if spec.width != win.spec.width || spec.height != win.spec.height {
                updates.push(CanvasEvent::Size(LogicalSize::new(spec.width as u32, spec.height as u32)));
            }

            if spec.left != win.spec.left || spec.top != win.spec.top {
                updates.push(CanvasEvent::Position(LogicalPosition::new(spec.left as i32, spec.top as i32)));
            }

            if spec.title != win.spec.title {
                updates.push(CanvasEvent::Title(spec.title.clone()));
            }

            if spec.visible != win.spec.visible {
                updates.push(CanvasEvent::Visible(spec.visible));
            }

            if spec.fullscreen != win.spec.fullscreen {
                updates.push(CanvasEvent::Fullscreen(spec.fullscreen));
            }

            if spec.cursor != win.spec.cursor {
                updates.push(CanvasEvent::Cursor(spec.cursor));
            }

            if spec.fit != win.spec.fit {
                updates.push(CanvasEvent::Fit(spec.fit));
            }

            if spec.background != win.spec.background {
                if let Some(color) = css_to_color(&spec.background) {
                    updates.push(CanvasEvent::Background(color));
                }
            }

            updates.push(CanvasEvent::Page(page));

            updates.drain(..).for_each(|event| {
                win.tx.send(Event::UserEvent(event)).ok();
            });

            win.spec = spec;
        }
    }

    pub fn capture_ui_event(&mut self, window_id:&WindowId, event:&WindowEvent){
        if let Some(mut win) = self.windows.iter_mut().find(|win| win.id == *window_id){
            win.sieve.capture(event, 1.0);
        }
    }

    pub fn use_ui_transform(&mut self, window_id:&WindowId, matrix:&Option<Matrix>){
        if let Some(mut win) = self.windows.iter_mut().find(|win| win.id == *window_id){
            if let Some(matrix) = matrix {
                win.sieve.use_transform(*matrix);
            }
        }
    }

    pub fn use_fullscreen_state(&mut self, window_id:&WindowId, is_fullscreen:bool){
        if let Some(mut win) = self.windows.iter_mut().find(|win| win.id == *window_id){
            win.spec.fullscreen = is_fullscreen;
        }
    }

    pub fn get_ui_changes(&mut self) -> serde_json::Value {
        let mut changes = serde_json::Map::new();
        self.windows.iter_mut().for_each(|win|{
            if let Some(payload) = win.sieve.serialize(){
                changes.insert(win.spec.id.clone(), payload);
            }
        });
        serde_json::json!(changes)
    }

    pub fn get_state(&mut self) -> serde_json::Value {
        let mut changes = serde_json::Map::new();
        self.windows.iter_mut().for_each(|win|{
            changes.insert(win.spec.id.clone(), json!(win.spec));
        });
        json!(changes)
    }

    pub fn len(&self) -> usize {
        self.windows.len()
    }
}



