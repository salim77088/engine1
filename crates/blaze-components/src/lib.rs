//! Blaze Engine — Built-in Components
//!
//! The component types every Blaze game uses. These are plain ECS
//! components (no logic) — systems in `blaze-render`, `blaze-physics`,
//! etc. read and write them.

use blaze_math::{Color, Vec3};
use serde::{Deserialize, Serialize};

/// Human-readable name shown in the hierarchy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Name(pub String);

impl Default for Name {
    fn default() -> Self { Self("Entity".into()) }
}

/// String tag for runtime lookup (`world.query::<&Tag>()`).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Tag(pub String);

/// Mesh primitive kind. The renderer maps this to a vertex/index buffer.
/// A future version will support loading arbitrary meshes from glTF.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum MeshPrimitive {
    Cube,
    Quad,
    Sphere,
    Plane,
}

impl Default for MeshPrimitive {
    fn default() -> Self { Self::Cube }
}

/// Mesh component — selects which primitive to render.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Mesh {
    pub primitive: MeshPrimitive,
}

/// PBR-ish material. `base_color` is linear RGBA; the renderer multiplies
/// it by the mesh's vertex color. Future versions will add textures,
/// roughness/metallic, and emissive.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Material {
    pub base_color: Color,
    pub roughness: f32,
    pub metallic: f32,
    /// Emissive color (added on top of lit shading).
    pub emissive: Color,
}

impl Default for Material {
    fn default() -> Self {
        Self {
            base_color: blaze_math::color::rgb(180, 180, 180),
            roughness: 0.8,
            metallic: 0.0,
            emissive: Color::new(0.0, 0.0, 0.0, 1.0),
        }
    }
}

/// 2D sprite. Drawn as a quad facing the camera, sized `size` in world units.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sprite {
    pub color: Color,
    pub size: Vec2,
    /// Future: TextureHandle. For now sprites are flat-colored quads.
    pub texture: Option<String>,
}

impl Default for Sprite {
    fn default() -> Self {
        Self {
            color: blaze_math::color::WHITE,
            size: Vec2::new(1.0, 1.0),
            texture: None,
        }
    }
}

/// Re-export Vec2 so users don't have to drag blaze_math in just for sprites.
pub use blaze_math::Vec2;

/// Camera component. The entity with a `Camera` component is the one
/// the renderer renders from. If multiple cameras exist, the first one
/// found (by entity id) is used.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Camera {
    pub fov_radians: f32,
    pub near: f32,
    pub far: f32,
    /// If true, renders as an orthographic camera (for 2D / UI).
    pub orthographic: bool,
    /// For orthographic cameras, half the viewport height in world units.
    pub ortho_size: f32,
    /// Background color used to clear the viewport when this camera renders.
    pub clear_color: Color,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            fov_radians: 60.0f32.to_radians(),
            near: 0.1,
            far: 1000.0,
            orthographic: false,
            ortho_size: 5.0,
            clear_color: blaze_math::color::rgb(30, 30, 34),
        }
    }
}

/// Directional light (like the sun). One should exist in the scene for
/// 3D lighting to work.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectionalLight {
    pub color: Color,
    pub intensity: f32,
    /// Direction the light travels (normalized automatically by the renderer).
    pub direction: Vec3,
}

impl Default for DirectionalLight {
    fn default() -> Self {
        Self {
            color: blaze_math::color::WHITE,
            intensity: 1.5,
            direction: Vec3::new(-0.4, -1.0, -0.3),
        }
    }
}

/// Point light with attenuation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointLight {
    pub color: Color,
    pub intensity: f32,
    pub range: f32,
}

impl Default for PointLight {
    fn default() -> Self {
        Self {
            color: blaze_math::color::WHITE,
            intensity: 10.0,
            range: 20.0,
        }
    }
}

/// Marks an entity as the editor's scene-view camera (used by the
/// editor to fly around the viewport, separate from the game camera).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EditorCamera;

// Re-export Transform2D for the sprite pipeline.
// (Already re-exported from blaze_math above — no need to duplicate.)

/// Component that marks an entity as a child of another entity.
/// (Stub — full parent-child transform chains come in a future version.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parent {
    pub entity_id: u64,
}

// Note: hecs auto-implements `Component` for any `Send + Sync + 'static`
// type, so we don't need to (and can't, due to orphan rules) manually
// impl `Component` for types defined in other crates like `Transform`.

