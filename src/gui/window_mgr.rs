use std::thread;
use serde_json::json;
use skia_safe::Matrix;
use crossbeam::channel::{self, Sender};
use serde_json::{Map, Value};
use winit::{
    dpi::{LogicalSize, LogicalPosition, PhysicalSize},
    event::WindowEvent,
    event_loop::{ActiveEventLoop},
    window::WindowId,
};

use crate::utils::css_to_color;
use crate::context::page::Page;
use super::event::{CanvasEvent, Sieve};
use super::window::{Window, WindowSpec};

#[derive(Default)]
pub struct WindowManager {
    windows: Vec<Window>,
    last: Option<LogicalPosition<f32>>,
}

impl WindowManager {

    pub fn add(&mut self, event_loop:&ActiveEventLoop, spec:WindowSpec, page:Page) {
        let mut window = Window::new(event_loop, spec, &page);

        // make sure mouse events use canvas-relative coordinates (in case win size doesn't match)
        window.update_fit();

        // cascade the windows based on the position of the most recently opened
        let dpr = window.handle.scale_factor();
        if let Ok(auto_loc) = window.handle.outer_position().map(|pt| pt.to_logical::<f32>(dpr)){
            if let Ok(inset) = window.handle.inner_position().map(|pt| pt.to_logical::<f32>(dpr)){
                let delta = inset.y - auto_loc.y;
                let reference = self.last.unwrap_or(auto_loc);
                let (left, top) = ( window.spec.left.unwrap_or(reference.x), window.spec.top.unwrap_or(reference.y) );

                window.handle.set_outer_position(LogicalPosition::new(left, top));
                window.handle.set_visible(true);

                window.spec.left = Some(left);
                window.spec.top = Some(top);
                self.last = Some(LogicalPosition::new(left + delta, top + delta));
            }
        }

        self.windows.push( window );
    }

    pub fn remove(&mut self, window_id:&WindowId){
        self.windows.retain(|win| win.id() != *window_id);
    }

    pub fn remove_by_token(&mut self, token:&str){
        self.windows.retain(|win| win.spec.id != token);
    }

    pub fn remove_all(&mut self){
        self.windows.clear();
    }

    pub fn update_window(&mut self, mut spec:WindowSpec, page:Page){
        if let Some(mut win) = self.windows.iter_mut().find(|win| win.spec.id == spec.id){
            if spec.width != win.spec.width || spec.height != win.spec.height {
                win.set_size(LogicalSize::new(spec.width as u32, spec.height as u32));
            }

            if let (Some(left), Some(top)) = (spec.left, spec.top){
                if spec.left != win.spec.left || spec.top != win.spec.top {
                    win.set_position(LogicalPosition::new(left as i32, top as i32));
                }
            }

            if spec.title != win.spec.title {
                win.set_title(&spec.title);
            }

            if spec.visible != win.spec.visible {
                win.set_visible(spec.visible);
            }

            if spec.fullscreen != win.spec.fullscreen {
                win.set_fullscreen(spec.fullscreen);
            }

            if spec.resizable != win.spec.resizable {
                win.set_resizable(spec.resizable);
            }

            if spec.resizable != win.spec.resizable {
                updates.push(CanvasEvent::Resizable(spec.resizable));
            }

            if spec.cursor != win.spec.cursor || spec.cursor_hidden != win.spec.cursor_hidden {
                let icon = if spec.cursor_hidden{ None }else{ Some(spec.cursor) };
                win.set_cursor(icon);
            }

            if spec.fit != win.spec.fit {
                win.set_fit(spec.fit);
            }

            if spec.background != win.spec.background {
                if let Some(color) = css_to_color(&spec.background) {
                    win.set_background(color);
                }else{
                    spec.background = win.spec.background.clone();
                }
            }

            win.set_page(page);

            win.spec = spec;
        }
    }

    pub fn find<F>(&mut self, id:&WindowId, mut f:F) where F:FnMut(&mut Window){
        self.windows.iter_mut().find(|win| win.id() == *id).map(f);
    }

    pub fn redraw(&mut self, id:&WindowId){
        self.find(id, |win| win.redraw() );
    }

    pub fn resized(&mut self, id:&WindowId, size:PhysicalSize<u32>){
        self.find(id, |win| win.did_resize(size) );
    }

    pub fn suspend_redraw(&mut self, id:&WindowId, flag:bool){
        self.find(id, |win| win.set_redrawing_suspended(flag) )
    }

    pub fn set_fullscreen_state(&mut self, id:&WindowId, is_fullscreen:bool){
        self.find(id, |win| win.set_fullscreen(is_fullscreen) )
    }

    pub fn capture_ui_event(&mut self, id:&WindowId, event:&WindowEvent){
        self.find(id, |win| win.sieve.capture(event) )
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



