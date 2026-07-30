//! Offscreen render target — owns a color texture + depth texture used to
//! render the game scene for the editor viewport.

use crate::DEPTH_FORMAT;

pub struct RenderTarget {
    pub width: u32,
    pub height: u32,
    color: wgpu::Texture,
    depth: wgpu::Texture,
}

impl RenderTarget {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat, width: u32, height: u32) -> Self {
        let width = width.max(1);
        let height = height.max(1);
        let color = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Blaze scene color"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let depth = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Blaze scene depth"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        Self { width, height, color, depth }
    }

    pub fn color_view(&self) -> wgpu::TextureView {
        self.color.create_view(&wgpu::TextureViewDescriptor::default())
    }

    /// Returns a `wgpu::TextureView` wrapped in an `Arc`-friendly type for
    /// registration with egui's `register_native_texture`. We use
    /// `wgpu::TextureView`'s `Arc`-compatibility via a small wrapper.
    pub fn color_view_owned(&self) -> Arc<wgpu::TextureView> {
        Arc::new(self.color.create_view(&wgpu::TextureViewDescriptor::default()))
    }

    pub fn depth_view(&self) -> wgpu::TextureView {
        self.depth.create_view(&wgpu::TextureViewDescriptor::default())
    }
}

use std::sync::Arc;
