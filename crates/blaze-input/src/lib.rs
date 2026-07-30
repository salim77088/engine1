//! Blaze Engine — Input
//!
//! Aggregates keyboard, mouse and (eventually) gamepad state into a single
//! `Input` resource that the game can poll each frame.

use blaze_core::{AppBuilder, Plugin};
use blaze_math::Vec2;
use parking_lot::RwLock;
use std::collections::HashSet;
use std::sync::Arc;

/// Logical keys. We mirror the small subset of `winit::event::VirtualKeyCode`
/// that games typically need, so the engine stays decoupled from winit's
/// version churn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    A, B, C, D, E, F, G, H, I, J, K, L, M,
    N, O, P, Q, R, S, T, U, V, W, X, Y, Z,
    Num0, Num1, Num2, Num3, Num4, Num5, Num6, Num7, Num8, Num9,
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,
    Up, Down, Left, Right,
    Space, Enter, Escape, Tab, Backspace, Delete, Insert, Home, End, PageUp, PageDown,
    LeftShift, RightShift, LeftCtrl, RightCtrl, LeftAlt, RightAlt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton { Left, Right, Middle, Other(u16) }

#[derive(Default)]
struct InputState {
    pressed_keys: HashSet<Key>,
    just_pressed_keys: HashSet<Key>,
    just_released_keys: HashSet<Key>,
    pressed_buttons: HashSet<MouseButton>,
    just_pressed_buttons: HashSet<MouseButton>,
    just_released_buttons: HashSet<MouseButton>,
    mouse_pos: Vec2,
    mouse_delta: Vec2,
    mouse_wheel: Vec2,
}

/// Shareable input resource. Stored as `Arc<RwLock<Input>>` in the
/// `Resources` table.
#[derive(Clone)]
pub struct Input {
    state: Arc<RwLock<InputState>>,
}

impl Default for Input {
    fn default() -> Self { Self { state: Arc::new(RwLock::new(InputState::default())) } }
}

impl Input {
    pub fn new() -> Self { Self::default() }

    // -------- public query API --------

    pub fn key(&self, k: Key) -> bool { self.state.read().pressed_keys.contains(&k) }
    pub fn key_pressed(&self, k: Key) -> bool { self.state.read().just_pressed_keys.contains(&k) }
    pub fn key_released(&self, k: Key) -> bool { self.state.read().just_released_keys.contains(&k) }

    pub fn mouse(&self, b: MouseButton) -> bool { self.state.read().pressed_buttons.contains(&b) }
    pub fn mouse_pressed(&self, b: MouseButton) -> bool { self.state.read().just_pressed_buttons.contains(&b) }
    pub fn mouse_released(&self, b: MouseButton) -> bool { self.state.read().just_released_buttons.contains(&b) }

    pub fn mouse_pos(&self) -> Vec2 { self.state.read().mouse_pos }
    pub fn mouse_delta(&self) -> Vec2 { self.state.read().mouse_delta }
    pub fn mouse_wheel(&self) -> Vec2 { self.state.read().mouse_wheel }

    // -------- event feed API (called by the windowing layer) --------

    pub fn process_key(&self, k: Key, pressed: bool) {
        let mut s = self.state.write();
        if pressed {
            if s.pressed_keys.insert(k) {
                s.just_pressed_keys.insert(k);
            }
        } else {
            s.pressed_keys.remove(&k);
            s.just_released_keys.insert(k);
        }
    }

    pub fn process_button(&self, b: MouseButton, pressed: bool) {
        let mut s = self.state.write();
        if pressed {
            if s.pressed_buttons.insert(b) {
                s.just_pressed_buttons.insert(b);
            }
        } else {
            s.pressed_buttons.remove(&b);
            s.just_released_buttons.insert(b);
        }
    }

    pub fn process_mouse_move(&self, pos: Vec2) {
        let mut s = self.state.write();
        s.mouse_delta = pos - s.mouse_pos;
        s.mouse_pos = pos;
    }

    pub fn process_mouse_wheel(&self, delta: Vec2) {
        self.state.write().mouse_wheel += delta;
    }

    /// Call at the end of every frame to flush just-pressed/released state.
    pub fn end_frame(&self) {
        let mut s = self.state.write();
        s.just_pressed_keys.clear();
        s.just_released_keys.clear();
        s.just_pressed_buttons.clear();
        s.just_released_buttons.clear();
        s.mouse_delta = Vec2::ZERO;
        s.mouse_wheel = Vec2::ZERO;
    }
}

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn name(&self) -> &str { "blaze-input" }
    fn build(&self, app: &mut AppBuilder) {
        app.insert_resource(Input::new());
    }
}
