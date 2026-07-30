//! Blaze Engine — Renderer
//!
//! Thin wgpu wrapper exposing a `Renderer` with a clear-color render pass
//! and a triangle pipeline. This is intentionally minimal: it gives the
//! engine a real, working 3D backend without dragging in heavy abstractions
//! that would dominate a small project. Future versions will grow mesh,
//! material and lighting pipelines on top of this base.

use anyhow::{Context, Result};
use blaze_math::Color;
use std::sync::Arc;
use wgpu::util::DeviceExt;

/// Configuration for creating a renderer surface.
pub struct SurfaceConfig {
    pub width: u32,
    pub height: u32,
}

/// Bundled wgpu handles. Stored as an `Arc<RenderContext>` resource.
pub struct RenderContext {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl RenderContext {
    /// Initialise a wgpu context. `compatible_surface` is optional; if
    /// present we tailor the adapter request to it.
    pub fn new(compatible_surface: Option<&wgpu::Surface<'static>>) -> Result<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface,
            force_fallback_adapter: false,
        }))
        .context("No suitable GPU adapter found")?;

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("Blaze GPU device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_defaults(),
                memory_hints: wgpu::MemoryHints::Performance,
            },
            None,
        ))
        .context("Failed to acquire GPU device")?;

        Ok(Self { instance, adapter, device, queue })
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    color: [f32; 3],
}

impl Vertex {
    const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3],
    };
}

const TRIANGLE_VERTICES: &[Vertex] = &[
    Vertex { position: [ 0.0,  0.5, 0.0], color: [1.0, 0.0, 0.0] },
    Vertex { position: [-0.5, -0.5, 0.0], color: [0.0, 1.0, 0.0] },
    Vertex { position: [ 0.5, -0.5, 0.0], color: [0.0, 0.0, 1.0] },
];

/// Renderer holding the GPU context, surface and a simple triangle pipeline.
pub struct Renderer {
    pub ctx: Arc<RenderContext>,
    pub surface: wgpu::Surface<'static>,
    pub surface_config: wgpu::SurfaceConfiguration,
    pub clear_color: Color,
    triangle_pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
}

impl Renderer {
    pub fn new(window: Arc<winit::window::Window>) -> Result<Self> {
        let ctx = Arc::new(RenderContext::new(None)?);
        let surface = ctx.instance
            .create_surface(window.clone())
            .context("create_surface")?;
        let caps = surface.get_capabilities(&ctx.adapter);
        let format = caps.formats.iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(caps.formats[0]);

        let size = window.inner_size();
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 2,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&ctx.device, &surface_config);

        let shader = ctx.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Blaze triangle shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("triangle.wgsl").into()),
        });

        let pipeline_layout = ctx.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Blaze triangle layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

        let triangle_pipeline = ctx.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Blaze triangle pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                compilation_options: Default::default(),
                buffers: &[Vertex::LAYOUT],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let vertex_buffer = ctx.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Blaze triangle vertices"),
            contents: bytemuck::cast_slice(TRIANGLE_VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });

        Ok(Self {
            ctx,
            surface,
            surface_config,
            clear_color: blaze_math::color::CORNFLOWER_BLUE,
            triangle_pipeline,
            vertex_buffer,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 { return; }
        self.surface_config.width = width;
        self.surface_config.height = height;
        self.surface.configure(&self.ctx.device, &self.surface_config);
    }

    pub fn render(&mut self) -> Result<()> {
        let frame = self.surface.get_current_texture().context("get_current_texture")?;
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self.ctx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Blaze encoder"),
        });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Blaze clear pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: self.clear_color.x as f64,
                            g: self.clear_color.y as f64,
                            b: self.clear_color.z as f64,
                            a: self.clear_color.w as f64,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.triangle_pipeline);
            pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            pass.draw(0..3, 0..1);
        }

        self.ctx.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
        Ok(())
    }
}
