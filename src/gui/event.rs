use skia_safe::Matrix;
use serde::Serialize;
use serde_json::json;
use winit::{
  dpi::{LogicalPosition, LogicalSize, PhysicalPosition},
  event::{ElementState, KeyEvent, Ime, Modifiers, MouseButton, MouseScrollDelta, WindowEvent},
  keyboard::{ModifiersState, KeyCode, KeyLocation, NamedKey, PhysicalKey::Code, Key::{Character, Named}},
};

use std::time::Instant;
use crate::context::page::Page;
use super::window::WindowSpec;

#[derive(Debug, Clone)]
pub enum AppEvent{
  Open(WindowSpec, Page),
  Close(u32),
  FrameRate(u64),
  Tick{ at: Instant },
  Quit,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum UiEvent{
  #[allow(non_snake_case)]
  Wheel{deltaX:f32, deltaY:f32},
  Move{left:f32, top:f32},
  Keyboard{event:String, key:String, code:KeyCode, location:u32, modifiers:ModifierKeys, repeat:bool},
  Composition{event:String, data:String},
  Mouse{event:String, button:Option<u16>, buttons:u16, point:LogicalPosition::<f32>, page_point:LogicalPosition::<f32>, modifiers:ModifierKeys},
  Input(Option<String>, String),
  Focus(bool),
  Resize(LogicalSize<u32>),
  Fullscreen(bool),
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModifierKeys{
  shift_key: bool,
  ctrl_key: bool,
  alt_key: bool,
  meta_key: bool,
}

impl From<ModifiersState> for ModifierKeys{
  fn from(state:ModifiersState) -> Self{
    ModifierKeys{
      shift_key: state.shift_key(),
      ctrl_key: state.control_key(),
      alt_key: state.alt_key(),
      meta_key: state.super_key(),
    }
  }
}

#[derive(Debug)]
pub struct Sieve{
  queue: Vec<UiEvent>,
  key_modifiers: ModifierKeys,
  mouse_point: PhysicalPosition::<f64>,
  mouse_button: Option<u16>,
  mouse_buttons: u16,
  mouse_transform: Matrix,
  compose_begun: bool,
  compose_ongoing: bool,
}

impl Sieve{
  pub fn new() -> Self {
    Sieve{
      queue: vec![],
      key_modifiers: Modifiers::default().state().into(),
      mouse_point: PhysicalPosition::default(),
      mouse_button: None,
      mouse_buttons: 0,
      mouse_transform: Matrix::new_identity(),
      compose_begun: false,
      compose_ongoing: false,
    }
  }

  pub fn use_transform(&mut self, matrix:Matrix){
    self.mouse_transform = matrix;
  }

  pub fn go_fullscreen(&mut self, is_full:bool){
    self.queue.push(UiEvent::Fullscreen(is_full));
  }

  fn add_mouse_event(&mut self, event:&str, dpr:f64){
    // helper to attach positions & keyboard modifiers for each type of mouse event
    let raw_position = LogicalPosition::<f32>::from_physical(self.mouse_point, dpr);
    let canvas_point = self.mouse_transform.map_point((raw_position.x, raw_position.y));
    let canvas_position = LogicalPosition::<f32>::new(canvas_point.x, canvas_point.y);

    let ui = UiEvent::Mouse{
      event: event.to_string(),
      point: canvas_position,
      page_point: raw_position,
      button: self.mouse_button,
      buttons: self.mouse_buttons,
      modifiers: self.key_modifiers,
    };

    // collapse runs of consecutive mousemoves (since the last vblank) to only keep the last one
    if event == "mousemove" && self.in_mousemove() {
      self.queue.pop();
    }
    self.queue.push(ui);
  }

  pub fn capture(&mut self, event:&WindowEvent, dpr:f64){
    match event{
      WindowEvent::Moved(physical_pt) => {
        let LogicalPosition{x, y} = physical_pt.to_logical(dpr);
        self.queue.push(UiEvent::Move{left:x, top:y});
      }

      WindowEvent::Resized(physical_size) => {
        let logical_size = LogicalSize::from_physical(*physical_size, dpr);
        self.queue.push(UiEvent::Resize(logical_size));
      }

      WindowEvent::Focused(in_focus) => {
        // losing focus while a button is held should cancel the ongoing drag
        if !*in_focus && self.mouse_buttons != 0 {
          self.mouse_button = None;
          self.mouse_buttons = 0;
          self.add_mouse_event("pointercancel", dpr);
        }
        self.queue.push(UiEvent::Focus(*in_focus));
      }

      WindowEvent::ModifiersChanged(modifiers) => {
        self.key_modifiers = modifiers.state().into();
      }

      WindowEvent::CursorEntered{..} => {
        self.add_mouse_event("mouseenter", dpr);
      }

      WindowEvent::CursorLeft{..} => {
        self.add_mouse_event("mouseleave", dpr);
      }

      WindowEvent::CursorMoved{position, ..} => {
        if *position != self.mouse_point{
          self.mouse_point = *position;
          self.add_mouse_event("mousemove", dpr);
        }
      }

      WindowEvent::MouseWheel{delta, ..} => {
        let LogicalPosition{x, y} = match delta {
          MouseScrollDelta::PixelDelta(physical_pt) => {
            LogicalPosition::from_physical(*physical_pt, dpr)
          },
          MouseScrollDelta::LineDelta(h, v) => {
            LogicalPosition{x:*h, y:*v}
          }
        };
        self.queue.push(UiEvent::Wheel{deltaX:x, deltaY:y});
      }

      WindowEvent::MouseInput{state, button, ..} => {
        let (button_id, button_bits) = match button {
          MouseButton::Left => (0, 1),
          MouseButton::Middle => (1, 4),
          MouseButton::Right => (2, 2),
          MouseButton::Back => (3, 8),
          MouseButton::Forward => (4, 16),
          MouseButton::Other(num) => (*num, 0),
        };

        self.mouse_button = Some(button_id);
        match state {
          ElementState::Pressed => {
            self.mouse_buttons |= button_bits;
            self.add_mouse_event("mousedown", dpr);
          },
          ElementState::Released => {
            self.mouse_buttons &= !button_bits;
            self.add_mouse_event("mouseup", dpr);
            self.mouse_button = None;
          },
        }
      }

      WindowEvent::KeyboardInput { event: KeyEvent {
          physical_key:Code(key_code), logical_key, state, repeat, location, ..
      }, .. } => {

        //
        // `keyup`/`keydown` events
        //
        let event_type = match state {
          ElementState::Pressed => "keydown",
          ElementState::Released => "keyup",
        }.to_string();

        let key_text = match logical_key{
          Named(n) => serde_json::from_value(json!(n)).unwrap(),
          Character(c) => c.to_string(),
          _ => String::new()
        };

        let key_location = match location{
          KeyLocation::Standard => 0,
          KeyLocation::Left => 1,
          KeyLocation::Right => 2,
          KeyLocation::Numpad => 3,
        };

        self.queue.push(UiEvent::Keyboard{
          event: event_type,
          key: key_text.clone(),
          code: key_code.clone(),
          location: key_location,
          modifiers: self.key_modifiers,
          repeat: *repeat
        });


        //
        // `input` events
        //
        if self.compose_ongoing{
          // don't emit the un-composed keystroke if it's part of an IME composition
          self.compose_ongoing = match state{
            ElementState::Released => false,
            _ => true,
          };
        }else{
          match state{
            // ignore keyups, just report presses & repeats
            ElementState::Pressed => {
              // in addition to printable characters, report spacing & deletion as input
              let key_char = match &logical_key{
                Character(c) => Some(c.to_string()),
                Named(NamedKey::Tab) => Some("\t".to_string()),
                Named(NamedKey::Space) => Some(" ".to_string()),
                Named(NamedKey::Backspace | NamedKey::Delete | NamedKey::Enter) => Some("".to_string()),
                _ => None
              };

              let input_type = match &logical_key{
                Named(NamedKey::Backspace) => "deleteContentBackward",
                Named(NamedKey::Delete) => "deleteContentForward",
                Named(NamedKey::Enter) => "insertLineBreak",
                _ => "insertText"
              }.to_string();

              if let Some(string) = key_char{
                let data = match !string.is_empty(){
                  true => Some(string),
                  false => None,
                };
                self.queue.push(UiEvent::Input(data, input_type));
              };
            },
            _ => {},
          }
        }
      }

      WindowEvent::Ime( event, ..) => {
        match &event {
          Ime::Preedit(string, Some(_range)) => {
            if !self.compose_begun{
              self.queue.push(UiEvent::Composition{
                event:"compositionstart".to_string(), data:"".to_string()
              });
              self.compose_begun = true; // flag: don't emit another `start` until this commits
            }
            self.queue.push(UiEvent::Composition {
              event:"compositionupdate".to_string(), data:string.clone()
            });
            self.compose_ongoing = true; // flag: don't emit `input` while composing
          },
          Ime::Commit(string) => {
            self.queue.push(UiEvent::Composition {
              event:"compositionend".to_string(), data:string.clone()
            });
            self.queue.push(UiEvent::Input(Some(string.clone()), "insertCompositionText".to_string())); // emit the composed character
            self.compose_begun = false;
          },
          _ => {}
        };
      }

      _ => {}
    }
  }

  pub fn collect(&mut self) -> serde_json::Value{
    let payload = json!(self.queue);
    self.queue.clear();
    payload
  }

  pub fn is_empty(&self) -> bool {
    self.queue.is_empty()
  }

  fn in_mousemove(&self) -> bool {
    matches!(self.queue.last(), Some(UiEvent::Mouse{ event, .. }) if event == "mousemove")
  }
}
