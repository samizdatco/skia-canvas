use std::thread;
use neon::prelude::*;
use skia_safe::{Matrix, Point, Color};
use crossbeam::channel::{self, Sender, Receiver};
use serde::Deserialize;
use winit::{
    dpi::{LogicalSize, LogicalPosition, PhysicalSize, PhysicalPosition},
    event::{Event, WindowEvent},
    event_loop::{EventLoopWindowTarget, EventLoopProxy},
    window::{Window as WinitWindow, WindowBuilder, WindowId},
};
#[cfg(target_os = "macos" )]
use winit::platform::macos::WindowExtMacOS;

use crate::gpu::{Renderer, runloop};
use crate::context::page::Page;
use super::event::{CanvasEvent, Sieve};

#[derive(Deserialize, Debug, Clone)]
pub struct WindowSpec {
    pub id: String,
    title: String,
    active: bool,
    loops: Option<bool>,
    visible: Option<bool>,
    fullscreen: bool,
    background: String,
    page: u32,
    width: f32,
    height: f32,
    cursor: String,
    fit: Option<Fit>,
    fps: u32,
    pub x: i32,
    pub y: i32,
}

#[derive(Copy, Clone, PartialEq, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Fit{
  ContainX,
  ContainY,
  Contain,
  Cover,
  Fill,
  ScaleDown
}

pub struct Window {
    pub handle: WinitWindow,
    pub proxy: EventLoopProxy<CanvasEvent>,
    pub renderer: Renderer,
    pub state: WindowSpec,
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
            .with_title(spec.title.clone())
            .build(&event_loop)
            .unwrap();
        let renderer = Renderer::for_window(&handle);

        Self{ handle, proxy, renderer, page:page.clone(), state:spec.clone() }
    }

    pub fn resize(&self, size: PhysicalSize<u32>){
        self.renderer.resize(size);
        self.proxy.send_event(CanvasEvent::Transform(
            self.handle.id(),
            self.fitting_matrix(false).invert()
        )).ok();
    }

    pub fn fitting_matrix(&self, scaled:bool) -> Matrix {
        let dpr = if scaled{ self.handle.scale_factor() } else { 1.0 };
        let size = self.handle.inner_size().to_logical::<f32>(dpr);
        let dims = self.page.bounds.size();
        let fit_x = size.width / dims.width;
        let fit_y = size.height / dims.height;

        let sf = match self.state.fit{
            Some(Fit::Cover) => fit_x.max(fit_y),
            Some(Fit::ScaleDown) => fit_x.min(fit_y).min(1.0),
            Some(Fit::Contain) => fit_x.min(fit_y),
            Some(Fit::ContainX) => fit_x,
            Some(Fit::ContainY) => fit_y,
            _ => 1.0
        };

        let (x_scale, y_scale) = match self.state.fit{
            Some(Fit::Fill) => (fit_x, fit_y),
            _ => (sf, sf)
        };

        let mut matrix = Matrix::scale((x_scale, y_scale));
        matrix.set_translate_x((size.width - dims.width * x_scale) / 2.0);
        matrix.set_translate_y((size.height - dims.height * y_scale) / 2.0);
        matrix
      }


    pub fn redraw(&mut self){
        runloop(|| {
            let matrix = self.fitting_matrix(true);
            let (clip, _) = matrix.map_rect(&self.page.bounds);

            self.renderer.draw(&self.handle, |canvas, _size| {
                canvas.clear(Color::BLACK);
                canvas.clip_rect(&clip, None, Some(true));
                canvas.draw_picture(self.page.get_picture(None).unwrap(), Some(&matrix), None);
            }).unwrap();
        })
    }

    pub fn handle_event(&mut self, event:Event<CanvasEvent>){
        runloop(|| {
            match event {
                Event::UserEvent(CanvasEvent::Page(page)) => {
                    self.page = page;
                    self.handle.request_redraw();
                }
                Event::WindowEvent { event, .. } => match event {
                    // WindowEvent::Moved { .. } => {
                    //     // We need to update our chosen video mode if the window
                    //     // was moved to an another monitor, so that the window
                    //     // appears on this monitor instead when we go fullscreen
                    //     let previous_video_mode = video_modes.get(video_mode_id).cloned();
                    //     video_modes = window.window.current_monitor().unwrap().video_modes().collect();
                    //     video_mode_id = video_mode_id.min(video_modes.len());
                    //     let video_mode = video_modes.get(video_mode_id);

                    //     // Different monitors may support different video modes,
                    //     // and the index we chose previously may now point to a
                    //     // completely different video mode, so notify the user
                    //     if video_mode != previous_video_mode.as_ref() {
                    //         println!(
                    //             "Window moved to another monitor, picked video mode: {}",
                    //             video_modes.get(video_mode_id).unwrap()
                    //         );
                    //     }
                    // },
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
    // offset: PhysicalPosition<i32>,
    last: Option<PhysicalPosition<i32>>,
}

impl Default for WindowManager {
    fn default() -> Self {
        Self{ windows: vec![], last: None }
    }
}

impl WindowManager {

    pub fn add(&mut self, mut window:Window){
        let id = window.handle.id();
        let (tx, rx) = channel::bounded(50);
        let mut sieve = Sieve::new(window.handle.scale_factor());
        if let Some(fit) = window.fitting_matrix(false).invert(){
            sieve.use_transform(fit);
        }

        // cascade the windows based on the position of the most recently opened
        let mut spec = window.state.clone();
        if let Ok(loc) = window.handle.outer_position(){
            if let Ok(inset) = window.handle.inner_position(){
                let delta = inset.y - loc.y;
                let corner = match self.last {
                    Some(last) => PhysicalPosition::new(last.x + delta, last.y + delta),
                    None => loc
                };
                spec.x = corner.x;
                spec.y = corner.y;
                self.last = Some(corner);
                window.handle.set_outer_position(corner);
            }
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

    pub fn send_event_for(&self, spec:&WindowSpec, event:Event<CanvasEvent>){
        if let Some(tx) = self.windows.iter().find(|win| win.spec.id == spec.id).map(|win| &win.tx){
            if let Some(event) = event.to_static() {
                tx.send(event).ok();
            }
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

    pub fn get_ui_changes(&mut self) -> serde_json::Value {
        let mut changes = serde_json::Map::new();
        self.windows.iter_mut().for_each(|win|{
            if let Some(payload) = win.sieve.serialize(){
                changes.insert(win.spec.id.clone(), payload);
            }
        });
        serde_json::json!(changes)
        // if !changes.is_empty(){
        //     println!("{}", serde_json::to_string(&changes).unwrap());
        // }
    }

    pub fn len(&self) -> usize {
        self.windows.len()
    }
}



