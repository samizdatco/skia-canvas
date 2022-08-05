use std::thread;
use serde_json::json;
use skia_safe::{Matrix, Color, Paint};
use crossbeam::channel::{self, Sender};
use serde::{Serialize, Deserialize};
use serde_json::{Map, Value};
use winit::{
    dpi::{LogicalSize, LogicalPosition, PhysicalSize, PhysicalPosition},
    event::{Event, WindowEvent},
    event_loop::{EventLoopWindowTarget, EventLoopProxy},
    window::{Window as WinitWindow, WindowBuilder, WindowId, CursorIcon, Fullscreen},
};
#[cfg(target_os = "macos" )]
use winit::platform::macos::WindowExtMacOS;

use crate::utils::{css_to_color, color_to_css};
use crate::gpu::{Renderer, runloop};
use crate::context::page::Page;
use super::event::{CanvasEvent, Sieve};

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WindowSpec {
    pub id: String,
    pub left: Option<f32>,
    pub top: Option<f32>,
    title: String,
    visible: bool,
    fullscreen: bool,
    background: String,
    page: u32,
    width: f32,
    height: f32,
    #[serde(with = "Cursor")]
    cursor: CursorIcon,
    cursor_hidden: bool,
    fit: Fit,
}

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Fit{
  None, ContainX, ContainY, Contain, Cover, Fill, ScaleDown, Resize
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

impl Window {
    pub fn new(event_loop:&EventLoopWindowTarget<CanvasEvent>, proxy:EventLoopProxy<CanvasEvent>, spec: &mut WindowSpec, page: &Page) -> Self {
        let size:LogicalSize<i32> = LogicalSize::new(spec.width as i32, spec.height as i32);
        let handle = WindowBuilder::new()
            .with_inner_size(size)
            .with_transparent(true)
            .with_title(spec.title.clone())
            .with_visible(false)
            .build(&event_loop)
            .unwrap();

        if let (Some(left), Some(top)) = (spec.left, spec.top){
            handle.set_outer_position(LogicalPosition::new(left, top));
        }

        let renderer = Renderer::for_window(&handle);
        let background = match css_to_color(&spec.background){
            Some(color) => color,
            None => {
                spec.background = "rgba(16,16,16,0.85)".to_string();
                css_to_color(&spec.background).unwrap()
            }
        };

        Self{ handle, proxy, renderer, page:page.clone(), fit:spec.fit, background }
    }

    pub fn resize(&self, size: PhysicalSize<u32>){
        if let Some(monitor) = self.handle.current_monitor(){
            self.renderer.resize(size);

            let id = self.handle.id();
            self.proxy.send_event(CanvasEvent::Transform(id, self.fitting_matrix().invert() )).ok();
            self.proxy.send_event(CanvasEvent::InFullscreen(id, monitor.size() == size )).ok();
        }
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
                            if let Some(icon) = icon{
                                self.handle.set_cursor_icon(icon);
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
                            let size = size.to_physical(self.handle.scale_factor());
                            self.handle.set_inner_size(size);
                            self.resize(size);
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
                        _ => { }
                    }
                }

                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::Resized(size) => {
                        self.resize(size);
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
    last: Option<LogicalPosition<f32>>,
}

impl Default for WindowManager {
    fn default() -> Self {
        Self{ windows: vec![], last: None }
    }
}

impl WindowManager {

    pub fn add(&mut self, event_loop:&EventLoopWindowTarget<CanvasEvent>, proxy:EventLoopProxy<CanvasEvent>, mut spec: WindowSpec, page: Page) {
        let mut window = Window::new(event_loop, proxy, &mut spec, &page);
        let id = window.handle.id();
        let (tx, rx) = channel::bounded(50);
        let mut sieve = Sieve::new(window.handle.scale_factor());
        if let Some(fit) = window.fitting_matrix().invert(){
            sieve.use_transform(fit);
        }

        // cascade the windows based on the position of the most recently opened
        let dpr = window.handle.scale_factor();
        if let Ok(auto_loc) = window.handle.outer_position().map(|pt| pt.to_logical::<f32>(dpr)){
            if let Ok(inset) = window.handle.inner_position().map(|pt| pt.to_logical::<f32>(dpr)){
                let delta = inset.y - auto_loc.y;
                let reference = self.last.unwrap_or(auto_loc);
                let (left, top) = ( spec.left.unwrap_or(reference.x), spec.top.unwrap_or(reference.y) );

                window.handle.set_outer_position(LogicalPosition::new(left, top));
                window.handle.set_visible(true);

                spec.left = Some(left);
                spec.top = Some(top);
                self.last = Some(LogicalPosition::new(left + delta, top + delta));
            }
        }

        self.windows.push( WindowRef{ id, spec, tx, sieve } );

        thread::spawn(move || {
            while let Ok(event) = rx.recv() {
                let mut queue = vec![event];
                while !rx.is_empty(){
                    queue.push(rx.recv().unwrap());
                }

                let mut needs_redraw = None;
                queue.drain(..).for_each(|event|{
                    match event {
                        Event::RedrawRequested(_) => needs_redraw = Some(event),
                        _ => window.handle_event(event)
                    }
                });

                if let Some(event) = needs_redraw {
                    window.handle_event(event)
                }
            }
        });
    }

    pub fn remove(&mut self, window_id:&WindowId){
        self.windows.retain(|win| win.id != *window_id);
    }

    pub fn remove_by_token(&mut self, token:&str){
        self.windows.retain(|win| win.spec.id != token);
    }

    pub fn send_event(&self, window_id:&WindowId, event:Event<CanvasEvent>){
        if let Some(tx) = self.windows.iter().find(|win| win.id == *window_id).map(|win| &win.tx){
            if let Some(event) = event.to_static() {
                tx.send(event).ok();
            }
        }
    }

    pub fn update_window(&mut self, mut spec:WindowSpec, page:Page){
        let mut updates:Vec<CanvasEvent> = vec![];

        if let Some(mut win) = self.windows.iter_mut().find(|win| win.spec.id == spec.id){
            if spec.width != win.spec.width || spec.height != win.spec.height {
                updates.push(CanvasEvent::Size(LogicalSize::new(spec.width as u32, spec.height as u32)));
            }

            if let (Some(left), Some(top)) = (spec.left, spec.top){
                if spec.left != win.spec.left || spec.top != win.spec.top {
                    updates.push(CanvasEvent::Position(LogicalPosition::new(left as i32, top as i32)));
                }
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

            if spec.cursor != win.spec.cursor || spec.cursor_hidden != win.spec.cursor_hidden {
                let icon = if spec.cursor_hidden{ None }else{ Some(spec.cursor) };
                updates.push(CanvasEvent::Cursor(icon));
            }

            if spec.fit != win.spec.fit {
                updates.push(CanvasEvent::Fit(spec.fit));
            }

            if spec.background != win.spec.background {
                if let Some(color) = css_to_color(&spec.background) {
                    updates.push(CanvasEvent::Background(color));
                }else{
                    spec.background = win.spec.background.clone();
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
        if let Some(win) = self.windows.iter_mut().find(|win| win.id == *window_id){
            win.sieve.capture(event);
        }
    }

    pub fn use_ui_transform(&mut self, window_id:&WindowId, matrix:&Option<Matrix>){
        if let Some(win) = self.windows.iter_mut().find(|win| win.id == *window_id){
            if let Some(matrix) = matrix {
                win.sieve.use_transform(*matrix);
            }
        }
    }

    pub fn set_fullscreen_state(&mut self, window_id:&WindowId, is_fullscreen:bool){
        if let Some(win) = self.windows.iter_mut().find(|win| win.id == *window_id){
            // tell the window to change state
            win.tx.send(Event::UserEvent(CanvasEvent::Fullscreen(is_fullscreen))).ok();
        }
        // and make sure the change is reflected in local state
        self.use_fullscreen_state(window_id, is_fullscreen);
    }

    pub fn use_fullscreen_state(&mut self, window_id:&WindowId, is_fullscreen:bool){
        if let Some(mut win) = self.windows.iter_mut().find(|win| win.id == *window_id){
            if win.spec.fullscreen != is_fullscreen{
                win.sieve.go_fullscreen(is_fullscreen);
                win.spec.fullscreen = is_fullscreen;
            }
        }
    }

    pub fn has_ui_changes(&self) -> bool {
        self.windows.iter().any(|win| !win.sieve.is_empty() )
    }

    pub fn get_ui_changes(&mut self) -> Value {
        let mut ui = Map::new();
        let mut state = Map::new();
        self.windows.iter_mut().for_each(|win|{
            if let Some(payload) = win.sieve.serialize(){
                ui.insert(win.spec.id.clone(), payload);
            }
            state.insert(win.spec.id.clone(), json!(win.spec));
        });
        json!({ "ui": ui, "state": state })
    }

    pub fn get_geometry(&mut self) -> serde_json::Value {
        let mut positions = serde_json::Map::new();
        self.windows.iter_mut().for_each(|win|{
            positions.insert(win.spec.id.clone(), json!({"left":win.spec.left, "top":win.spec.top}));
        });
        json!(positions)
    }

    pub fn len(&self) -> usize {
        self.windows.len()
    }
}



