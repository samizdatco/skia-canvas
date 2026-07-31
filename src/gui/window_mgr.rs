use std::time::Duration;
use serde_json::json;
use serde_json::{Map, Value};
use winit::{
    dpi::{LogicalSize, LogicalPosition},
    event_loop::ActiveEventLoop,
    event::WindowEvent,
    monitor::MonitorHandle,
    window::WindowId,
};

use crate::utils::css_to_color;
use crate::gfx::page::Page;
use super::window::{Window, WindowSpec};

#[derive(Default)]
pub struct WindowManager {
    windows: Vec<Window>,
    last: Option<LogicalPosition<f32>>,
    vsync_monitor: Option<MonitorHandle>, // display the vblank source was keyed to at last check
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

    pub fn remove_by_token(&mut self, token:u32){
        self.windows.retain(|win| win.spec.id != token);
    }

    pub fn remove_all(&mut self){
        self.windows.clear();
    }
    pub fn update_window(&mut self, mut spec:WindowSpec, page:Page){
        if let Some(win) = self.windows.iter_mut().find(|win| win.spec.id == spec.id){
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
                win.sieve.go_fullscreen(spec.fullscreen);
            }

            if spec.resizable != win.spec.resizable {
                win.set_resizable(spec.resizable);
            }

            if spec.borderless != win.spec.borderless {
                win.set_borderless(spec.borderless);
            }

            if spec.cursor != win.spec.cursor {
                win.set_cursor(&spec.cursor);
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

    pub fn find<F>(&mut self, id:&WindowId, f:F) where F:FnMut(&mut Window){
        self.windows.iter_mut().find(|win| win.id() == *id).map(f);
    }

    pub fn request_redraw_all(&self){
        // used to kick Wayland's redraw-driven frame-callback loop (incl. newly-opened windows)
        self.windows.iter().for_each(|win| win.handle.request_redraw());
    }

    pub fn has_ui_changes(&self) -> bool {
        self.windows.iter().any(|win| !win.sieve.is_empty() )
    }

    pub fn any_present_pending(&self) -> bool {
        // true while any window has a requested redraw that hasn't presented yet. vblank sources will
        // defer the next roundtrip until the last frame is complete to avoid stalling
        self.windows.iter().any(|win| win.is_present_pending() )
    }

    pub fn get_ui_changes(&mut self) -> Value {
        let mut ui = Map::new();
        let mut state = Map::new();
        self.windows.iter_mut().for_each(|win|{
            // collect new UI events
            if !win.sieve.is_empty(){
                ui.insert(win.spec.id.to_string(), win.sieve.collect());
            }
            state.insert(win.spec.id.to_string(), json!(win.spec));
        });
        json!({ "ui": ui, "state": state })
    }

    pub fn get_geometry(&mut self) -> Value {
        let mut positions = Map::new();
        self.windows.iter_mut().for_each(|win|{
            positions.insert(win.spec.id.to_string(), json!({"left":win.spec.left, "top":win.spec.top}));
        });
        json!({"geom":positions})
    }

    pub fn is_empty(&self) -> bool {
        self.windows.len() == 0
    }

    // compare the first window's current monitor (which drives the vblank source) to its cached value and
    // flag when they differ (but ignore the transient None that can occur when dragging between monitors)
    pub fn main_monitor_changed(&mut self) -> bool {
        let current = self.windows.first().and_then(|win| win.monitor.clone());
        let changed = matches!((&self.vsync_monitor, &current), (Some(prev), Some(now)) if prev != now);
        if current.is_some(){
            self.vsync_monitor = current;
        }
        changed
    }

    pub fn refresh_interval(&self) -> Duration {
        // pace against the first window's monitor; fall back to 60Hz when the platform
        // can't report a refresh rate (multi-monitor phase handling is future work)
        let hz = self.windows.first()
            .and_then(|win| win.handle.current_monitor())
            .and_then(|monitor| monitor.refresh_rate_millihertz())
            .map(|millihertz| millihertz as f64 / 1000.0)
            .filter(|hz| *hz > 0.0)
            .unwrap_or(60.0);
        Duration::from_secs_f64(1.0 / hz)
    }

    #[cfg(target_os = "macos")]
    pub fn primary_display_id(&self) -> Option<u32> {
        // CGDirectDisplayID of the first window's monitor, for CVDisplayLink
        use winit::platform::macos::MonitorHandleExtMacOS;
        self.windows.first()
            .and_then(|win| win.handle.current_monitor())
            .map(|monitor| monitor.native_id())
    }
}
