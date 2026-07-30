//! Mesh pipeline — renders lit 3D meshes (cube, quad, sphere, plane)
//! with a PBR-ish material (base color, roughness, metallic, emissive)
//! lit by one directional light and up to 8 point lights.

use blaze_components::{DirectionalLight, Material, MeshPrimitive, PointLight};
use blaze_math::Vec3;
use crate::camera::CameraUniform;
use bytemuck::{Pod, Zeroable};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct MeshVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 4],
}

impl MeshVertex {
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<MeshVertex>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &wgpu::vertex_attr_array![
            0 => Float32x3, // position
            1 => Float32x3, // normal
            2 => Float32x4, // color
        ],
    };
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct ModelUniform {
    model: [[f32; 4]; 4],
    base_color: [f32; 4],
    roughness: f32,
    metallic: f32,
    emissive: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable, Default)]
struct LightsUniform {
    dir_dir: [f32; 4],       // xyz + pad
    dir_color: [f32; 4],     // rgb + intensity
    dir_intensity: f32,
    _pad0: [f32; 3],
    // Up to 8 point lights.
    point_pos: [[f32; 4]; 8],
    point_color: [[f32; 4]; 8],
    point_count: f32,
    _pad1: [f32; 3],
}

pub struct MeshPipeline {
    pipeline: wgpu::RenderPipeline,
    bind_layout: wgpu::BindGroupLayout,
    camera_buf: wgpu::Buffer,
    camera_bind: wgpu::BindGroup,
    lights_buf: wgpu::Buffer,
    lights_bind: wgpu::BindGroup,
    /// Cached vertex+index buffers for each primitive.
    primitives: HashMap<MeshPrimitive, PrimitiveMesh>,
    /// Per-entity model uniform buffer + bind group.
    /// Keyed by entity id (u32).
    model_bufs: Arc<RwLock<HashMap<u32, (wgpu::Buffer, wgpu::BindGroup)>>>,
}

struct PrimitiveMesh {
    vertex_buf: wgpu::Buffer,
    index_buf: wgpu::Buffer,
    index_count: u32,
}

impl MeshPipeline {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Blaze mesh shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("mesh.wgsl").into()),
        });

        // Camera bind group (binding 0).
        let camera_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Blaze camera buf"),
            size: std::mem::size_of::<CameraUniform>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let camera_layout = wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };

        // Lights bind group (binding 0 of a separate layout).
        let lights_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Blaze lights buf"),
            size: std::mem::size_of::<LightsUniform>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Model bind group layout (binding 1: model uniform).
        let model_layout = wgpu::BindGroupLayoutEntry {
            binding: 1,
            visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };

        let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Blaze mesh bind layout"),
            entries: &[camera_layout, model_layout, wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let camera_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Blaze camera bind"),
            layout: &bind_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buf.as_entire_binding(),
            }],
        });

        let lights_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Blaze lights bind"),
            layout: &bind_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 2,
                resource: lights_buf.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Blaze mesh pipeline layout"),
            bind_group_layouts: &[&bind_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Blaze mesh pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                compilation_options: Default::default(),
                buffers: &[MeshVertex::LAYOUT],
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
            primitive: wgpu::PrimitiveState {
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: crate::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Build primitive meshes.
        let mut primitives = HashMap::new();
        primitives.insert(MeshPrimitive::Cube, make_cube(device));
        primitives.insert(MeshPrimitive::Quad, make_quad(device));
        primitives.insert(MeshPrimitive::Plane, make_plane(device));
        primitives.insert(MeshPrimitive::Sphere, make_sphere(device, 16, 16));

        Self {
            pipeline,
            bind_layout,
            camera_buf,
            camera_bind,
            lights_buf,
            lights_bind,
            primitives,
            model_bufs: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Update the camera uniform buffer + lights uniform buffer for this frame.
    pub fn prepare_frame(
        &self,
        _device: &wgpu::Device,
        queue: &wgpu::Queue,
        view_proj: glam::Mat4,
        dir_light: Option<&DirectionalLight>,
        point_lights: &[(Vec3, PointLight)],
    ) {
        let cam = CameraUniform {
            view_proj: view_proj.to_cols_array_2d(),
            view_pos: [0.0; 4],
            _pad: [0.0; 4],
        };
        queue.write_buffer(&self.camera_buf, 0, bytemuck::cast_slice(&[cam]));

        let mut lights = LightsUniform::default();
        if let Some(d) = dir_light {
            let n = d.direction.normalize_or_zero();
            lights.dir_dir = [n.x, n.y, n.z, 0.0];
            lights.dir_color = [d.color.x, d.color.y, d.color.z, 1.0];
            lights.dir_intensity = d.intensity;
        }
        for (i, (pos, pl)) in point_lights.iter().take(8).enumerate() {
            lights.point_pos[i] = [pos.x, pos.y, pos.z, 0.0];
            lights.point_color[i] = [pl.color.x, pl.color.y, pl.color.z, pl.intensity];
        }
        lights.point_count = point_lights.len().min(8) as f32;
        queue.write_buffer(&self.lights_buf, 0, bytemuck::cast_slice(&[lights]));
    }

    /// Draw a single mesh entity. Lazily creates (and caches) a per-entity
    /// model uniform buffer + bind group keyed by entity id.
    pub fn draw_mesh(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        entity: blaze_ecs::Entity,
        primitive: MeshPrimitive,
        material: &Material,
        model: glam::Mat4,
    ) {
        let Some(prim) = self.primitives.get(&primitive) else { return; };

        let key = entity.id();
        let mut bufs = self.model_bufs.write();
        let entry = bufs.entry(key).or_insert_with(|| {
            let buf = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Blaze model buf"),
                size: std::mem::size_of::<ModelUniform>() as wgpu::BufferAddress,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Blaze model bind"),
                layout: &self.bind_layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: self.camera_buf.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: buf.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 2, resource: self.lights_buf.as_entire_binding() },
                ],
            });
            (buf, bind)
        });

        let m = ModelUniform {
            model: model.to_cols_array_2d(),
            base_color: [material.base_color.x, material.base_color.y, material.base_color.z, material.base_color.w],
            roughness: material.roughness,
            metallic: material.metallic,
            emissive: [material.emissive.x, material.emissive.y, material.emissive.z, 1.0],
        };
        queue.write_buffer(&entry.0, 0, bytemuck::cast_slice(&[m]));

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &entry.1, &[]);
        pass.set_vertex_buffer(0, prim.vertex_buf.slice(..));
        pass.set_index_buffer(prim.index_buf.slice(..), wgpu::IndexFormat::Uint16);
        pass.draw_indexed(0..prim.index_count, 0, 0..1);
    }
}

fn make_cube(device: &wgpu::Device) -> PrimitiveMesh {
    // 24 vertices (4 per face), 36 indices.
    let white: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
    let verts: Vec<MeshVertex> = vec![
        // +X
        MeshVertex { position: [ 0.5, -0.5, -0.5], normal: [ 1.0, 0.0, 0.0], color: white },
        MeshVertex { position: [ 0.5,  0.5, -0.5], normal: [ 1.0, 0.0, 0.0], color: white },
        MeshVertex { position: [ 0.5,  0.5,  0.5], normal: [ 1.0, 0.0, 0.0], color: white },
        MeshVertex { position: [ 0.5, -0.5,  0.5], normal: [ 1.0, 0.0, 0.0], color: white },
        // -X
        MeshVertex { position: [-0.5, -0.5,  0.5], normal: [-1.0, 0.0, 0.0], color: white },
        MeshVertex { position: [-0.5,  0.5,  0.5], normal: [-1.0, 0.0, 0.0], color: white },
        MeshVertex { position: [-0.5,  0.5, -0.5], normal: [-1.0, 0.0, 0.0], color: white },
        MeshVertex { position: [-0.5, -0.5, -0.5], normal: [-1.0, 0.0, 0.0], color: white },
        // +Y
        MeshVertex { position: [-0.5,  0.5, -0.5], normal: [0.0,  1.0, 0.0], color: white },
        MeshVertex { position: [-0.5,  0.5,  0.5], normal: [0.0,  1.0, 0.0], color: white },
        MeshVertex { position: [ 0.5,  0.5,  0.5], normal: [0.0,  1.0, 0.0], color: white },
        MeshVertex { position: [ 0.5,  0.5, -0.5], normal: [0.0,  1.0, 0.0], color: white },
        // -Y
        MeshVertex { position: [-0.5, -0.5,  0.5], normal: [0.0, -1.0, 0.0], color: white },
        MeshVertex { position: [-0.5, -0.5, -0.5], normal: [0.0, -1.0, 0.0], color: white },
        MeshVertex { position: [ 0.5, -0.5, -0.5], normal: [0.0, -1.0, 0.0], color: white },
        MeshVertex { position: [ 0.5, -0.5,  0.5], normal: [0.0, -1.0, 0.0], color: white },
        // +Z
        MeshVertex { position: [-0.5, -0.5,  0.5], normal: [0.0, 0.0,  1.0], color: white },
        MeshVertex { position: [ 0.5, -0.5,  0.5], normal: [0.0, 0.0,  1.0], color: white },
        MeshVertex { position: [ 0.5,  0.5,  0.5], normal: [0.0, 0.0,  1.0], color: white },
        MeshVertex { position: [-0.5,  0.5,  0.5], normal: [0.0, 0.0,  1.0], color: white },
        // -Z
        MeshVertex { position: [ 0.5, -0.5, -0.5], normal: [0.0, 0.0, -1.0], color: white },
        MeshVertex { position: [-0.5, -0.5, -0.5], normal: [0.0, 0.0, -1.0], color: white },
        MeshVertex { position: [-0.5,  0.5, -0.5], normal: [0.0, 0.0, -1.0], color: white },
        MeshVertex { position: [ 0.5,  0.5, -0.5], normal: [0.0, 0.0, -1.0], color: white },
    ];
    let indices: Vec<u16> = vec![
        0, 1, 2, 0, 2, 3,
        4, 5, 6, 4, 6, 7,
        8, 9, 10, 8, 10, 11,
        12, 13, 14, 12, 14, 15,
        16, 17, 18, 16, 18, 19,
        20, 21, 22, 20, 22, 23,
    ];
    let vertex_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Blaze cube verts"),
        contents: bytemuck::cast_slice(&verts),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let index_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Blaze cube indices"),
        contents: bytemuck::cast_slice(&indices),
        usage: wgpu::BufferUsages::INDEX,
    });
    PrimitiveMesh { vertex_buf, index_buf, index_count: indices.len() as u32 }
}

fn make_quad(device: &wgpu::Device) -> PrimitiveMesh {
    let white: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
    let verts: Vec<MeshVertex> = vec![
        MeshVertex { position: [-0.5, -0.5, 0.0], normal: [0.0, 0.0, 1.0], color: white },
        MeshVertex { position: [ 0.5, -0.5, 0.0], normal: [0.0, 0.0, 1.0], color: white },
        MeshVertex { position: [ 0.5,  0.5, 0.0], normal: [0.0, 0.0, 1.0], color: white },
        MeshVertex { position: [-0.5,  0.5, 0.0], normal: [0.0, 0.0, 1.0], color: white },
    ];
    let indices: Vec<u16> = vec![0, 1, 2, 0, 2, 3];
    let vertex_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Blaze quad verts"),
        contents: bytemuck::cast_slice(&verts),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let index_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Blaze quad indices"),
        contents: bytemuck::cast_slice(&indices),
        usage: wgpu::BufferUsages::INDEX,
    });
    PrimitiveMesh { vertex_buf, index_buf, index_count: indices.len() as u32 }
}

fn make_plane(device: &wgpu::Device) -> PrimitiveMesh {
    // A 10x10 plane on the XZ axis.
    let s = 5.0;
    let white: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
    let verts: Vec<MeshVertex> = vec![
        MeshVertex { position: [-s, 0.0, -s], normal: [0.0, 1.0, 0.0], color: white },
        MeshVertex { position: [ s, 0.0, -s], normal: [0.0, 1.0, 0.0], color: white },
        MeshVertex { position: [ s, 0.0,  s], normal: [0.0, 1.0, 0.0], color: white },
        MeshVertex { position: [-s, 0.0,  s], normal: [0.0, 1.0, 0.0], color: white },
    ];
    let indices: Vec<u16> = vec![0, 1, 2, 0, 2, 3];
    let vertex_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Blaze plane verts"),
        contents: bytemuck::cast_slice(&verts),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let index_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Blaze plane indices"),
        contents: bytemuck::cast_slice(&indices),
        usage: wgpu::BufferUsages::INDEX,
    });
    PrimitiveMesh { vertex_buf, index_buf, index_count: indices.len() as u32 }
}

fn make_sphere(device: &wgpu::Device, segments: u32, rings: u32) -> PrimitiveMesh {
    let mut verts: Vec<MeshVertex> = Vec::new();
    let mut indices: Vec<u16> = Vec::new();
    let white: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
    for ring in 0..=rings {
        let theta = std::f32::consts::PI * ring as f32 / rings as f32;
        let s = theta.sin();
        let c = theta.cos();
        for seg in 0..=segments {
            let phi = 2.0 * std::f32::consts::PI * seg as f32 / segments as f32;
            let x = s * phi.cos();
            let y = c;
            let z = s * phi.sin();
            verts.push(MeshVertex {
                position: [x * 0.5, y * 0.5, z * 0.5],
                normal: [x, y, z],
                color: white,
            });
        }
    }
    for ring in 0..rings {
        for seg in 0..segments {
            let a = ring * (segments + 1) + seg;
            let b = a + segments + 1;
            indices.push(a as u16);
            indices.push(b as u16);
            indices.push((a + 1) as u16);
            indices.push(b as u16);
            indices.push((b + 1) as u16);
            indices.push((a + 1) as u16);
        }
    }
    let vertex_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Blaze sphere verts"),
        contents: bytemuck::cast_slice(&verts),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let index_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Blaze sphere indices"),
        contents: bytemuck::cast_slice(&indices),
        usage: wgpu::BufferUsages::INDEX,
    });
    PrimitiveMesh { vertex_buf, index_buf, index_count: indices.len() as u32 }
}
