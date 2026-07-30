//! Blaze Engine — Math
//!
//! Thin facade over [`glam`] that exposes the types most games need, plus a
//! few helper constructors and a transform type used across the engine.

pub use glam::{
    Mat2, Mat3, Mat4, Quat, Vec2, Vec3, Vec4,
    UVec2, UVec3, UVec4, IVec2, IVec3, IVec4,
};

/// Translation + rotation + scale, the bread-and-butter of every scene graph.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }
}

impl Transform {
    pub const IDENTITY: Self = Self {
        translation: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
    };

    pub fn from_translation(t: Vec3) -> Self {
        Self { translation: t, ..Default::default() }
    }

    pub fn from_scale(s: Vec3) -> Self {
        Self { scale: s, ..Default::default() }
    }

    pub fn from_rotation(r: Quat) -> Self {
        Self { rotation: r, ..Default::default() }
    }

    pub fn translate(&mut self, by: Vec3) { self.translation += by; }

    pub fn to_matrix(&self) -> Mat4 {
        Mat4::from_translation(self.translation)
            * Mat4::from_quat(self.rotation)
            * Mat4::from_scale(self.scale)
    }

    pub fn lerp(&self, other: &Self, t: f32) -> Self {
        Self {
            translation: self.translation.lerp(other.translation, t),
            rotation: self.rotation.slerp(other.rotation, t),
            scale: self.scale.lerp(other.scale, t),
        }
    }
}

/// 2D transform variant for 2D games and UI layouts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform2D {
    pub translation: Vec2,
    pub rotation: f32,
    pub scale: Vec2,
}

impl Default for Transform2D {
    fn default() -> Self {
        Self { translation: Vec2::ZERO, rotation: 0.0, scale: Vec2::ONE }
    }
}

impl Transform2D {
    pub fn to_matrix(&self) -> Mat3 {
        Mat3::from_translation(self.translation)
            * Mat3::from_angle(self.rotation)
            * Mat3::from_scale(self.scale)
    }
}

/// Color in linear RGBA, 0..1.
pub type Color = Vec4;

pub mod color {
    use crate::Color;

    pub const WHITE:   Color = Color::new(1.0, 1.0, 1.0, 1.0);
    pub const BLACK:   Color = Color::new(0.0, 0.0, 0.0, 1.0);
    pub const RED:     Color = Color::new(1.0, 0.0, 0.0, 1.0);
    pub const GREEN:   Color = Color::new(0.0, 1.0, 0.0, 1.0);
    pub const BLUE:    Color = Color::new(0.0, 0.0, 1.0, 1.0);
    pub const YELLOW:  Color = Color::new(1.0, 1.0, 0.0, 1.0);
    pub const CYAN:    Color = Color::new(0.0, 1.0, 1.0, 1.0);
    pub const MAGENTA: Color = Color::new(1.0, 0.0, 1.0, 1.0);
    pub const CORNFLOWER_BLUE: Color = Color::new(0.39, 0.58, 0.93, 1.0);

    /// Convert sRGB 0..255 to linear 0..1.
    pub fn rgb(r: u8, g: u8, b: u8) -> Color {
        Color::new(
            (r as f32 / 255.0).powf(2.2),
            (g as f32 / 255.0).powf(2.2),
            (b as f32 / 255.0).powf(2.2),
            1.0,
        )
    }

    pub fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color {
        let mut c = rgb(r, g, b);
        c.w = a as f32 / 255.0;
        c
    }
}
