use neon::prelude::*;
use skia_safe::{Matrix, Point};
use std::{
    collections::{HashMap},
    time::{Duration, Instant},
};
use winit::{
  dpi::{LogicalSize, LogicalPosition, PhysicalSize},
  event_loop::{ControlFlow},
  event::{WindowEvent, ElementState,  KeyboardInput, VirtualKeyCode, ModifiersState, MouseButton, MouseScrollDelta},
  window::{CursorIcon},
};

use crate::context::page::Page;
use super::window::WindowSpec;

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum Fit{
  Contain{x:bool, y:bool},
  Cover,
  Fill,
  ScaleDown
}

#[derive(Debug, Clone)]
pub enum CanvasEvent{
  Open(WindowSpec, Page),
  Close(String),
  Page(Page),
  Title(String),
  FrameRate(u64),
  Fullscreen(bool),
  InFullscreen(bool),
  Visible(bool),
  Cursor(Option<CursorIcon>),
  CursorVisible(bool),
  Fit(Option<Fit>),
  Position(LogicalPosition<i32>),
  Size(LogicalSize<u32>),
  Resized(PhysicalSize<u32>),
  Transform(Option<Matrix>),
  Heartbeat,
  Render,
  Quit,
}

#[derive(Debug)]
pub enum UiEvent{
  Keyboard{event:String, key:String, code:u32, repeat:bool},
  Input(char),
  Mouse(String),
  Wheel(LogicalPosition<f64>),
  Position(LogicalPosition<i32>),
  Size(LogicalSize<u32>),
  Fullscreen(bool),
}

#[derive(Debug)]
pub struct Sieve{
  queue: Vec<UiEvent>,
  key_modifiers: ModifiersState,
  key_repeats: HashMap<VirtualKeyCode, i32>,
  mouse_point: LogicalPosition::<i32>,
  mouse_button: Option<u16>,
  mouse_transform: Matrix,
}

impl Sieve{
  pub fn new() -> Self {
    Sieve{
      queue: vec![],
      key_modifiers: ModifiersState::empty(),
      key_repeats: HashMap::new(),
      mouse_point: LogicalPosition::<i32>{x:0, y:0},
      mouse_button: None,
      mouse_transform: Matrix::new_identity(),
    }
  }

  pub fn use_transform(&mut self, matrix:Matrix){
    self.mouse_transform = matrix;
  }

  pub fn go_fullscreen(&mut self, is_full:bool){
    self.queue.push(UiEvent::Fullscreen(is_full));
    self.key_repeats.clear(); // keyups don't get delivered during the transition apparently?
  }

  pub fn is_empty(&self) -> bool{
    self.queue.len() == 0
  }

  pub fn capture(&mut self, event:&WindowEvent, dpr:f64){
    match event{
      WindowEvent::Moved(physical_pt) => {
        let logical_pt:LogicalPosition<i32> = LogicalPosition::from_physical(*physical_pt, dpr);
        self.queue.push(UiEvent::Position(logical_pt));
      }

      WindowEvent::Resized(physical_size) => {
        let logical_size:LogicalSize<u32> = LogicalSize::from_physical(*physical_size, dpr);
        self.queue.push(UiEvent::Size(logical_size));
      }

      WindowEvent::ModifiersChanged(state) => {
        self.key_modifiers = *state;
      }

      WindowEvent::ReceivedCharacter(character) => {
        self.queue.push(UiEvent::Input(*character));
      }

      WindowEvent::CursorEntered{..} => {
        let mouse_event = "mouseenter".to_string();
        self.queue.push(UiEvent::Mouse(mouse_event));
      }

      WindowEvent::CursorLeft{..} => {
        let mouse_event = "mouseleave".to_string();
        self.queue.push(UiEvent::Mouse(mouse_event));
      }

      WindowEvent::CursorMoved{position, ..} => {
        let Point{x, y} = self.mouse_transform.map_point((position.x as f32, position.y as f32));
        self.mouse_point = LogicalPosition::new(x as i32, y as i32);

        let mouse_event = "mousemove".to_string();
        self.queue.push(UiEvent::Mouse(mouse_event));
      }

      WindowEvent::MouseWheel{delta, ..} => {
        let dxdy:LogicalPosition<f64> = match delta {
          MouseScrollDelta::PixelDelta(physical_pt) => {
            LogicalPosition::from_physical(*physical_pt, dpr)
          },
          MouseScrollDelta::LineDelta(h, v) => {
            LogicalPosition::<f64>{x:*h as f64, y:*v as f64}
          }
        };
        self.queue.push(UiEvent::Wheel(dxdy));
      }

      WindowEvent::MouseInput{state, button, ..} => {
        let mouse_event = match state {
          ElementState::Pressed => "mousedown",
          ElementState::Released => "mouseup"
        }.to_string();

        self.mouse_button = match button {
          MouseButton::Left => Some(0),
          MouseButton::Middle => Some(1),
          MouseButton::Right => Some(2),
          MouseButton::Other(num) => Some(*num)
        };
        self.queue.push(UiEvent::Mouse(mouse_event));
      }

      WindowEvent::KeyboardInput { input:
        KeyboardInput { scancode, state, virtual_keycode: Some(keycode), ..}, ..
      } => {
        let (event_type, count) = match state{
          ElementState::Pressed => {
            let count = self.key_repeats.entry(*keycode).or_insert(-1);
            *count += 1;
            ("keydown", *count)
          },
          ElementState::Released => {
            self.key_repeats.remove(&keycode);
            ("keyup", 0)
          }
        };

        if event_type == "keyup" || count < 2{
          self.queue.push(UiEvent::Keyboard{
            event: event_type.to_string(),
            key: serde_json::to_string(keycode).unwrap(),
            code: *scancode,
            repeat: count > 0
          });
        }

      }
      _ => {}
    }
  }

  pub fn serialized<'a>(&mut self, cx: &mut FunctionContext<'a>) -> Vec<Handle<'a, JsValue>>{
    let mut payload:Vec<Handle<JsValue>> = (0..17).map(|_|
      //   0–5: x, y, w, h, fullscreen, [alt, ctrl, meta, shift]
      //  6–10: input, keyEvent, key, code, repeat,
      // 11–14: [mouseEvents], mouseX, mouseY, button,
      // 15–16: wheelX, wheelY
      cx.undefined().upcast::<JsValue>()
    ).collect();

    let mut include_mods = false;
    let mut mouse_events = vec![];

    for change in &self.queue {
      match change{
        UiEvent::Position(LogicalPosition{x, y}) => {
          payload[0] = cx.number(*x).upcast::<JsValue>(); // x
          payload[1] = cx.number(*y).upcast::<JsValue>(); // y
        }
        UiEvent::Size(LogicalSize{width, height}) => {
          payload[2] = cx.number(*width).upcast::<JsValue>();  // width
          payload[3] = cx.number(*height).upcast::<JsValue>(); // height
        }
        UiEvent::Fullscreen(flag) => {
          payload[4] = cx.boolean(*flag).upcast::<JsValue>(); // fullscreen
        }
        UiEvent::Input(character) => {
          include_mods = true;
          payload[6] = cx.string(character.to_string()).upcast::<JsValue>(); // input
        }
        UiEvent::Keyboard{event, key, code, repeat} => {
          include_mods = true;
          payload[7] = cx.string(event).upcast::<JsValue>();     // keyup | keydown
          payload[8] = cx.string(key).upcast::<JsValue>();       // key
          payload[9] = cx.number(*code).upcast::<JsValue>();     // code
          payload[10] = cx.boolean(*repeat).upcast::<JsValue>(); // repeat
        }
        UiEvent::Mouse(event_type) => {
          include_mods = true;
          let event_name = cx.string(event_type).upcast::<JsValue>();
          mouse_events.push(event_name);
        }
        UiEvent::Wheel(delta) => {
          payload[15] = cx.number(delta.x).upcast::<JsValue>(); // wheelX
          payload[16] = cx.number(delta.y).upcast::<JsValue>(); // wheelY
        }
      }
    }

    if !mouse_events.is_empty(){
      let event_list = JsArray::new(cx, mouse_events.len() as u32);
      for (i, obj) in mouse_events.iter().enumerate() {
        event_list.set(cx, i as u32, *obj).unwrap();
      }
      payload[11] = event_list.upcast::<JsValue>();

      let LogicalPosition{x, y} = self.mouse_point;
      payload[12] = cx.number(x).upcast::<JsValue>(); // mouseX
      payload[13] = cx.number(y).upcast::<JsValue>(); // mouseY

      if let Some(button_id) = self.mouse_button{
        payload[14] = cx.number(button_id).upcast::<JsValue>(); // button
        self.mouse_button = None;
      }
    }

    if include_mods{
      let mod_info = JsArray::new(cx, 4);
      let mod_info_vec = vec![
        cx.boolean(self.key_modifiers.alt()).upcast::<JsValue>(),   // altKey
        cx.boolean(self.key_modifiers.ctrl()).upcast::<JsValue>(),  // ctrlKey
        cx.boolean(self.key_modifiers.logo()).upcast::<JsValue>(),  // metaKey
        cx.boolean(self.key_modifiers.shift()).upcast::<JsValue>(), // shiftKey
      ];
      for (i, obj) in mod_info_vec.iter().enumerate() {
          mod_info.set(cx, i as u32, *obj).unwrap();
      }
      payload[5] = mod_info.upcast::<JsValue>();
    }

    self.queue.clear();
    payload
  }
}

pub struct Cadence{
  rate: u64,
  last: Instant,
  wakeup: Duration,
  render: Duration,
  begun: bool,
}

impl Default for Cadence {
  fn default() -> Self {
    Self{
      rate: 0,
      last: Instant::now(),
      render: Duration::new(0, 0),
      wakeup: Duration::new(0, 0),
      begun: false,
    }
  }
}

impl Cadence{
  fn on_startup<F:FnOnce()>(&mut self, init:F){
    if self.begun{ return }
    self.begun = true;
    init();
  }

  pub fn set_frame_rate(&mut self, rate:u64){
    let frame_time = 1_000_000_000/rate.max(1);
    let watch_interval = 1_000_000.max(frame_time/10);
    self.render = Duration::from_nanos(frame_time);
    self.wakeup = Duration::from_nanos(frame_time - watch_interval);
    self.rate = rate;
  }

  pub fn on_next_frame<F:Fn()>(&mut self, draw:F) -> ControlFlow{
    if self.rate == 0{
      return ControlFlow::Wait;
    }

    if self.last.elapsed() >= self.render{
      while self.last < Instant::now() - self.render{
        self.last += self.render
      }
      draw();
    }

    match self.last.elapsed() < self.wakeup {
      true => ControlFlow::WaitUntil(self.last + self.wakeup),
      false => ControlFlow::Poll,
    }
  }

  pub fn active(&self) -> bool{
    self.rate > 0
  }
}

