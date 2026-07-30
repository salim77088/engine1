//! Camera uniform buffer layout, shared between the mesh and sprite pipelines.

use bytemuck::{Pod, Zeroable};

/// Std140-compatible uniform buffer sent to the vertex shader.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct CameraUniform {
    pub view_proj: [[f32; 4]; 4],
    pub view_pos: [f32; 4],
    pub _pad: [f32; 4],
}

impl Default for CameraUniform {
    fn default() -> Self {
        Self {
            view_proj: glam::Mat4::IDENTITY.to_cols_array_2d(),
            view_pos: [0.0, 0.0, 0.0, 0.0],
            _pad: [0.0; 4],
        }
    }
}
