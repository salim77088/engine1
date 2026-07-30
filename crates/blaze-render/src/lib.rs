//! Blaze Engine — Renderer
//!
//! A real wgpu renderer with:
//!   * A camera component (view + projection matrices).
//!   * A lit mesh pipeline (PBR-ish: directional + point lights).
//!   * A 2D sprite pipeline (textured quads).
//!   * Render-to-texture mode for the editor viewport (so the game scene
//!     can be displayed inside an egui CentralPanel).
//!   * egui overlay rendering.
//!
//! The render graph each frame:
//!   1. Acquire the surface texture.
//!   2. If in editor mode: render the scene to an offscreen `RenderTarget`,
//!      then blit that target onto the surface as a full-screen quad inside
//!      the egui central panel.
//!   3. If in standalone mode: render the scene directly to the surface.
//!   4. Paint the egui jobs on top.
//!   5. Present.

pub mod camera;
pub mod mesh;
pub mod sprite;
pub mod target;

use anyhow::{Context, Result};
use blaze_assets::SharedAssetRegistry;
use blaze_components::{Camera, DirectionalLight, Material, Mesh, PointLight, Sprite};
use blaze_ecs::{SharedWorld, World};
use blaze_math::{Color, Transform, Vec3};
use egui_wgpu::ScreenDescriptor;
use parking_lot::RwLock;
use std::sync::Arc;
use wgpu::util::DeviceExt;

pub use camera::CameraUniform;
pub use mesh::{MeshPipeline, MeshVertex};
pub use sprite::{SpritePipeline, SpriteVertex};
pub use target::RenderTarget;

/// Bundled wgpu handles.
pub struct RenderContext {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl RenderContext {
    pub fn new() -> Result<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
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

/// The depth format used everywhere.
pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// The renderer. Owns the GPU context, the surface, the mesh/sprite
/// pipelines and an offscreen render target used for the editor viewport.
pub struct Renderer {
    pub ctx: Arc<RenderContext>,
    pub surface: wgpu::Surface<'static>,
    pub surface_config: wgpu::SurfaceConfiguration,
    pub clear_color: Color,

    pub mesh_pipeline: MeshPipeline,
    pub sprite_pipeline: SpritePipeline,

    /// Offscreen render target used when rendering the game scene for the
    /// editor viewport. Recreated on resize.
    pub scene_target: RenderTarget,

    /// egui <-> wgpu bridge.
    pub egui_renderer: egui_wgpu::Renderer,

    /// Whether we're rendering in "editor" mode (scene to texture, then
    /// blitted into the egui central panel) or "standalone" mode (scene
    /// directly to the surface).
    pub editor_mode: bool,
}

impl Renderer {
    pub fn new(window: Arc<winit::window::Window>) -> Result<Self> {
        let ctx = Arc::new(RenderContext::new()?);
        let surface = ctx.instance.create_surface(window.clone()).context("create_surface")?;
        let caps = surface.get_capabilities(&ctx.adapter);
        let format = caps.formats.iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(caps.formats[0]);

        let size = window.inner_size();
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::TEXTURE_BINDING,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 2,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&ctx.device, &surface_config);

        let mesh_pipeline = MeshPipeline::new(&ctx.device, format);
        let sprite_pipeline = SpritePipeline::new(&ctx.device, format);

        let scene_target = RenderTarget::new(&ctx.device, format, size.width.max(1), size.height.max(1));

        let egui_renderer = egui_wgpu::Renderer::new(&ctx.device, format, None, 1, false);

        Ok(Self {
            ctx,
            surface,
            surface_config,
            clear_color: blaze_math::color::rgb(30, 30, 34),
            mesh_pipeline,
            sprite_pipeline,
            scene_target,
            egui_renderer,
            editor_mode: true,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 { return; }
        self.surface_config.width = width;
        self.surface_config.height = height;
        self.surface.configure(&self.ctx.device, &self.surface_config);
        self.scene_target = RenderTarget::new(
            &self.ctx.device,
            self.surface_config.format,
            width,
            height,
        );
    }

    /// Render the entire frame: scene pass + egui pass.
    ///
    /// In editor mode the scene is rendered to `scene_target`, and the
    /// egui central panel receives a `SceneTargetImage` it blits into
    /// the viewport area. In standalone mode the scene is rendered
    /// directly to the surface and the egui pass paints over it.
    pub fn render_frame(
        &mut self,
        world: &World,
        paint_jobs: &[egui::epaint::ClippedPrimitive],
        screen_descriptor: &ScreenDescriptor,
        textures_delta: &egui::TexturesDelta,
        scene_image_id: egui::TextureId,
    ) -> Result<()> {
        let frame = self.surface.get_current_texture().context("get_current_texture")?;
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self.ctx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Blaze encoder"),
        });

        if self.editor_mode {
            // 1. Render the scene to the offscreen target.
            self.render_scene(world, &mut encoder, SceneTarget::Offscreen)?;

            // 2. Update the egui texture for the scene image with the
            //    offscreen target's content. The egui_wgpu renderer lets
            //    us register a user texture backed by a wgpu::TextureView.
            //    We do this once at startup (see `register_scene_texture`)
            //    and just keep the binding pointed at scene_target.view.

            // 3. egui pass — paint onto the surface view (Load).
            for (id, image_delta) in &textures_delta.set {
                self.egui_renderer.update_texture(
                    &self.ctx.device,
                    &self.ctx.queue,
                    *id,
                    image_delta,
                );
            }
            self.egui_renderer.update_buffers(
                &self.ctx.device,
                &self.ctx.queue,
                &mut encoder,
                paint_jobs,
                screen_descriptor,
            );
            {
                let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Blaze egui pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                self.egui_renderer.render(&mut pass.forget_lifetime(), paint_jobs, screen_descriptor);
            }
            for id in &textures_delta.free {
                self.egui_renderer.free_texture(id);
            }
            let _ = scene_image_id; // already registered at startup
        } else {
            // Standalone: render scene directly to surface, then egui on top.
            self.render_scene(world, &mut encoder, SceneTarget::Surface(&view))?;

            for (id, image_delta) in &textures_delta.set {
                self.egui_renderer.update_texture(
                    &self.ctx.device,
                    &self.ctx.queue,
                    *id,
                    image_delta,
                );
            }
            self.egui_renderer.update_buffers(
                &self.ctx.device,
                &self.ctx.queue,
                &mut encoder,
                paint_jobs,
                screen_descriptor,
            );
            {
                let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Blaze egui pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                self.egui_renderer.render(&mut pass.forget_lifetime(), paint_jobs, screen_descriptor);
            }
            for id in &textures_delta.free {
                self.egui_renderer.free_texture(id);
            }
        }

        self.ctx.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
        Ok(())
    }

    /// Render the scene (meshes + sprites + lights) to the given target.
    fn render_scene(&mut self, world: &World, encoder: &mut wgpu::CommandEncoder, target: SceneTarget<'_>) -> Result<()> {
        // 1. Pick the active camera entity (first entity with a Camera component).
        let (cam_entity, cam_data, cam_transform) = {
            let mut iter = world.query::<(&Camera, &Transform)>();
            match iter.iter().next() {
                Some((e, (c, t))) => (e, c.clone(), *t),
                None => {
                    // No camera — clear and return.
                    self.clear_only(encoder, target)?;
                    return Ok(());
                }
            }
        };
        let _ = cam_entity;

        // 2. Build view + projection matrices.
        let view = glam::Mat4::look_at_rh(
            cam_transform.translation,
            cam_transform.translation + cam_transform.rotation * glam::Vec3::Z * -1.0, // forward
            glam::Vec3::Y,
        );
        let aspect = self.scene_target.width as f32 / self.scene_target.height.max(1) as f32;
        let proj = if cam_data.orthographic {
            glam::Mat4::orthographic_rh(
                -cam_data.ortho_size * aspect,
                cam_data.ortho_size * aspect,
                -cam_data.ortho_size,
                cam_data.ortho_size,
                cam_data.near,
                cam_data.far,
            )
        } else {
            glam::Mat4::perspective_rh(cam_data.fov_radians, aspect, cam_data.near, cam_data.far)
        };
        let view_proj = proj * view;

        // 3. Collect lights.
        let dir_light = world.query::<&DirectionalLight>().iter().next().map(|(_, l)| l.clone());
        let point_lights: Vec<(Vec3, PointLight)> = world
            .query::<(&PointLight, &Transform)>()
            .iter()
            .map(|(_, (l, t))| (t.translation, l.clone()))
            .collect();

        // 4. Render meshes.
        // Need to pick the right color view: offscreen target's view (owned)
        // or the surface view (borrowed). We work around TextureView not
        // being Clone by branching on the target type with two code paths.
        let depth_view = self.scene_target.depth_view();
        match target {
            SceneTarget::Offscreen => {
                let color_view = self.scene_target.color_view();
                self.render_scene_into(encoder, &color_view, &depth_view, world, view_proj, &cam_data, dir_light.as_ref(), &point_lights)?;
            }
            SceneTarget::Surface(v) => {
                self.render_scene_into(encoder, v, &depth_view, world, view_proj, &cam_data, dir_light.as_ref(), &point_lights)?;
            }
        }
        Ok(())
    }

    fn render_scene_into(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        color_view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        world: &World,
        view_proj: glam::Mat4,
        cam_data: &Camera,
        dir_light: Option<&DirectionalLight>,
        point_lights: &[(Vec3, PointLight)],
    ) -> Result<()> {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Blaze scene pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: color_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: cam_data.clear_color.x as f64,
                        g: cam_data.clear_color.y as f64,
                        b: cam_data.clear_color.z as f64,
                        a: cam_data.clear_color.w as f64,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        // Mesh pass.
        self.mesh_pipeline.prepare_frame(&self.ctx.device, &self.ctx.queue, view_proj, dir_light, point_lights);
        for (entity, (mesh, material, transform)) in world.query::<(&Mesh, &Material, &Transform)>().iter() {
            let model = transform.to_matrix();
            self.mesh_pipeline.draw_mesh(&mut pass, &self.ctx.device, &self.ctx.queue, entity, mesh.primitive, material, model);
        }

        // Sprite pass.
        self.sprite_pipeline.prepare_frame(&self.ctx.device, &self.ctx.queue, view_proj);
        for (entity, (sprite, transform)) in world.query::<(&Sprite, &Transform)>().iter() {
            let model = transform.to_matrix();
            self.sprite_pipeline.draw_sprite(&mut pass, &self.ctx.device, &self.ctx.queue, entity, sprite, model, view_proj);
        }
        Ok(())
    }

    fn clear_only(&self, encoder: &mut wgpu::CommandEncoder, target: SceneTarget<'_>) -> Result<()> {
        let view_owned;
        let view: &wgpu::TextureView = match target {
            SceneTarget::Offscreen => {
                view_owned = self.scene_target.color_view();
                &view_owned
            }
            SceneTarget::Surface(v) => v,
        };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Blaze clear pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.12, g: 0.12, b: 0.14, a: 1.0 }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        let _ = pass;
        Ok(())
    }

    /// Register the offscreen scene target's color view as an egui user
    /// texture. Returns the `egui::TextureId` you should pass to
    /// `egui::Image::from_texture` to display the game viewport.
    pub fn register_scene_texture(&mut self, _egui_ctx: &egui::Context) -> egui::TextureId {
        let view = self.scene_target.color_view();
        self.egui_renderer.register_native_texture(
            &self.ctx.device,
            &view,
            wgpu::FilterMode::Linear,
        )
    }

    /// Update the scene texture binding after a resize. The old id is
    /// stale; callers should re-call `register_scene_texture`.
    pub fn reregister_scene_texture(&mut self, _egui_ctx: &egui::Context) -> egui::TextureId {
        self.register_scene_texture(_egui_ctx)
    }
}

enum SceneTarget<'a> {
    Offscreen,
    Surface(&'a wgpu::TextureView),
}
