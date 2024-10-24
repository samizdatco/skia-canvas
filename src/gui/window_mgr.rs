use std::thread;
use serde_json::json;
use skia_safe::Matrix;
use crossbeam::channel::{self, Sender};
use serde_json::{Map, Value};
use winit::{
    dpi::{LogicalSize, LogicalPosition},
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoopProxy},
    window::WindowId,
};

use crate::utils::css_to_color;
use crate::context::page::Page;
use super::event::{CanvasEvent, Sieve};
use super::window::{Window, WindowSpec};

struct WindowRef { tx: Sender<CanvasEvent>, id: WindowId, spec: WindowSpec, sieve:Sieve }

#[derive(Default)]
pub struct WindowManager {
    windows: Vec<WindowRef>,
    last: Option<LogicalPosition<f32>>,
}

impl WindowManager {

    pub fn add(&mut self, event_loop:&ActiveEventLoop, proxy:EventLoopProxy<CanvasEvent>, mut spec: WindowSpec, page: Page) {
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

        // hold a reference to the window on the main thread…
        self.windows.push( WindowRef{ id, spec, tx, sieve } );

        // …but let the window's event handler & renderer run on a background thread
        thread::spawn(move || {
            while let Ok(event) = rx.recv() {
                let mut queue = vec![event];
                while !rx.is_empty(){
                    queue.push(rx.recv().unwrap());
                }

                let mut needs_redraw = None;
                queue.drain(..).for_each(|event|{
                    match event {
                        CanvasEvent::RedrawRequested => needs_redraw = Some(event),
                        _ => window.handle_event(event)
                    }
                });

                // deduplicate and defer redraw requests until all other events were handled
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

    pub fn send_event(&self, window_id:&WindowId, event:CanvasEvent){
        if let Some(tx) = self.windows.iter().find(|win| win.id == *window_id).map(|win| &win.tx){
            tx.send(event).ok();
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
                win.tx.send(event).ok();
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
            win.tx.send(CanvasEvent::Fullscreen(is_fullscreen)).ok();
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

    pub fn get_geometry(&mut self) -> Value {
        let mut positions = Map::new();
        self.windows.iter_mut().for_each(|win|{
            positions.insert(win.spec.id.clone(), json!({"left":win.spec.left, "top":win.spec.top}));
        });
        json!({"geom":positions})
    }

    pub fn len(&self) -> usize {
        self.windows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.windows.len() == 0
    }
}



