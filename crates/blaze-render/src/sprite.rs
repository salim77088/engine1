//! Sprite pipeline — renders 2D colored quads with an optional texture.

use blaze_components::Sprite;
use bytemuck::{Pod, Zeroable};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct SpriteVertex {
    pub position: [f32; 3],
    pub color: [f32; 4],
    pub uv: [f32; 2],
}

impl SpriteVertex {
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<SpriteVertex>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &wgpu::vertex_attr_array![
            0 => Float32x3, // position
            1 => Float32x4, // color
            2 => Float32x2, // uv
        ],
    };
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct SpriteUniform {
    view_proj: [[f32; 4]; 4],
    model: [[f32; 4]; 4],
    color: [f32; 4],
    _pad: [f32; 4],
}

pub struct SpritePipeline {
    pipeline: wgpu::RenderPipeline,
    bind_layout: wgpu::BindGroupLayout,
    /// Per-entity uniform buffer + bind group. Keyed by entity id.
    uniforms: Arc<RwLock<HashMap<u32, (wgpu::Buffer, wgpu::BindGroup)>>>,
    quad_verts: wgpu::Buffer,
    quad_indices: wgpu::Buffer,
}

impl SpritePipeline {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Blaze sprite shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("sprite.wgsl").into()),
        });

        let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Blaze sprite bind layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Blaze sprite pipeline layout"),
            bind_group_layouts: &[&bind_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Blaze sprite pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                compilation_options: Default::default(),
                buffers: &[SpriteVertex::LAYOUT],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: crate::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Quad vertices (unit quad centered at origin, +Z forward).
        let white: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
        let verts: Vec<SpriteVertex> = vec![
            SpriteVertex { position: [-0.5, -0.5, 0.0], color: white, uv: [0.0, 1.0] },
            SpriteVertex { position: [ 0.5, -0.5, 0.0], color: white, uv: [1.0, 1.0] },
            SpriteVertex { position: [ 0.5,  0.5, 0.0], color: white, uv: [1.0, 0.0] },
            SpriteVertex { position: [-0.5,  0.5, 0.0], color: white, uv: [0.0, 0.0] },
        ];
        let indices: Vec<u16> = vec![0, 1, 2, 0, 2, 3];
        let quad_verts = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Blaze sprite quad verts"),
            contents: bytemuck::cast_slice(&verts),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let quad_indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Blaze sprite quad indices"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Self {
            pipeline,
            bind_layout,
            uniforms: Arc::new(RwLock::new(HashMap::new())),
            quad_verts,
            quad_indices,
        }
    }

    pub fn prepare_frame(&self, _device: &wgpu::Device, _queue: &wgpu::Queue, _view_proj: glam::Mat4) {
        // Nothing to do here — view_proj is baked into each entity's uniform
        // in draw_sprite so the pipeline only needs one bind group per entity.
    }

    pub fn draw_sprite(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        entity: blaze_ecs::Entity,
        sprite: &Sprite,
        model: glam::Mat4,
        view_proj: glam::Mat4,
    ) {
        // Bake the per-entity scale into the model matrix.
        let model = model * glam::Mat4::from_scale(glam::Vec3::new(sprite.size.x, sprite.size.y, 1.0));

        let key = entity.id();
        let mut uniforms = self.uniforms.write();
        let entry = uniforms.entry(key).or_insert_with(|| {
            let buf = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Blaze sprite uniform buf"),
                size: std::mem::size_of::<SpriteUniform>() as wgpu::BufferAddress,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Blaze sprite bind"),
                layout: &self.bind_layout,
                entries: &[wgpu::BindGroupEntry { binding: 0, resource: buf.as_entire_binding() }],
            });
            (buf, bind)
        });
        let u = SpriteUniform {
            view_proj: view_proj.to_cols_array_2d(),
            model: model.to_cols_array_2d(),
            color: [sprite.color.x, sprite.color.y, sprite.color.z, sprite.color.w],
            _pad: [0.0; 4],
        };
        queue.write_buffer(&entry.0, 0, bytemuck::cast_slice(&[u]));

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &entry.1, &[]);
        pass.set_vertex_buffer(0, self.quad_verts.slice(..));
        pass.set_index_buffer(self.quad_indices.slice(..), wgpu::IndexFormat::Uint16);
        pass.draw_indexed(0..6, 0, 0..1);
    }
}
