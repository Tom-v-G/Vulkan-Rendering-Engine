use std::collections::{HashMap, HashSet};
use winit::{
    event::{ElementState, KeyEvent},
    keyboard::{KeyCode, PhysicalKey},
};

use crate::{app::RenderApp, input};

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Action {
    DecreaseModels,
    IncreaseModels,

    MoveForward,
    MoveBackward,
    MoveLeft,
    MoveRight,
    MoveUp,
    MoveDown,

    SetMenuMode,
    Quit,
}

impl Action {
    const CONTINUOUS_PRESS: [Action; 6] = [
        Action::MoveForward,
        Action::MoveBackward,
        Action::MoveLeft,
        Action::MoveRight,
        Action::MoveUp,
        Action::MoveDown,
    ];

    const SINGLE_PRESS: [Action; 2] = [Action::SetMenuMode, Action::Quit];
}

pub struct InputState {
    pub continuous_pressed_keys: HashSet<KeyCode>,
    pub single_pressed_keys: HashSet<KeyCode>,
    pub mouse_delta: (f64, f64),
}

impl InputState {
    pub fn new() -> Self {
        let continuous_pressed_keys: HashSet<KeyCode> = HashSet::new();
        let single_pressed_keys: HashSet<KeyCode> = HashSet::new();
        let mouse_delta = (0., 0.);
        return Self {
            continuous_pressed_keys,
            single_pressed_keys,
            mouse_delta,
        };
    }
}

pub struct InputMap {
    pub bindings: HashMap<KeyCode, Action>,
}

impl InputMap {
    pub fn new() -> Self {
        let mut bindings = HashMap::new();

        bindings.insert(KeyCode::ArrowLeft, Action::DecreaseModels);
        bindings.insert(KeyCode::ArrowRight, Action::IncreaseModels);

        bindings.insert(KeyCode::KeyW, Action::MoveForward);
        bindings.insert(KeyCode::KeyS, Action::MoveBackward);
        bindings.insert(KeyCode::KeyA, Action::MoveLeft);
        bindings.insert(KeyCode::KeyD, Action::MoveRight);
        bindings.insert(KeyCode::Space, Action::MoveUp);
        bindings.insert(KeyCode::ShiftLeft, Action::MoveDown);

        bindings.insert(KeyCode::Escape, Action::Quit);
        bindings.insert(KeyCode::KeyE, Action::SetMenuMode);

        Self { bindings }
    }

    pub fn get_action(&self, key: &KeyCode) -> Option<Action> {
        self.bindings.get(key).copied()
    }
}

pub fn handle_keyboard_input(event: KeyEvent, input_state: &mut InputState, input_map: &InputMap) {
    if let PhysicalKey::Code(code) = event.physical_key {
        match event.state {
            ElementState::Pressed => {
                if Action::CONTINUOUS_PRESS.contains(input_map.bindings.get(&code).unwrap()) {
                    input_state.continuous_pressed_keys.insert(code);
                } else if Action::SINGLE_PRESS.contains(input_map.bindings.get(&code).unwrap()) {
                    input_state.single_pressed_keys.insert(code);
                }
            }
            ElementState::Released => {
                if Action::CONTINUOUS_PRESS.contains(input_map.bindings.get(&code).unwrap()) {
                    input_state.continuous_pressed_keys.remove(&code);
                }
            }
            _ => {}
        }
    }
}

pub fn handle_mouse_input(delta: (f64, f64), input_state: &mut InputState) {
    input_state.mouse_delta = delta;
}
