use neon::prelude::*;
use skia_safe::{Matrix, Color};
use serde::Deserialize;
use winit::{
    dpi::{LogicalSize, LogicalPosition, PhysicalSize},
    event::{Event, WindowEvent},
    event_loop::{EventLoopWindowTarget},
    window::{Window as WinitWindow, WindowBuilder},
};
#[cfg(target_os = "macos" )]
use winit::platform::macos::WindowExtMacOS;

use crate::gpu::{Renderer, runloop};
use crate::context::page::Page;
use super::event::CanvasEvent;

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
    pub renderer: Renderer,
    pub state: WindowSpec,
    pub page: Page
}

impl Finalize for Window {}

impl Window {
    pub fn new(event_loop:&EventLoopWindowTarget<CanvasEvent>, spec: &WindowSpec, page: Page) -> Self {
        let size:LogicalSize<i32> = LogicalSize::new(spec.width as i32, spec.height as i32);
        let loc:LogicalPosition<i32> = LogicalPosition::new(500+spec.x, 300+spec.y);
        let handle = WindowBuilder::new()
            .with_inner_size(size)
            .with_position(loc)
            .with_title(spec.title.clone())
            .build(&event_loop)
            .unwrap();
        let renderer = Renderer::for_window(&handle);
        Self{ handle, renderer, page, state:spec.clone() }
    }


    pub fn resize(&self, size: PhysicalSize<u32>){
        runloop(|| {
            self.renderer.resize(size);
            self.handle.request_redraw();
        })
    }

    pub fn fitting_matrix(&self) -> Matrix {
        let dpr = self.handle.scale_factor();
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
            let matrix = self.fitting_matrix();
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