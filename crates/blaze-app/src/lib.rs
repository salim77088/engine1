//! Blaze Engine — App integration
//!
//! The default runner:
//!   1. Opens a winit window.
//!   2. Initialises the renderer (with offscreen scene target + egui overlay).
//!   3. Each frame:
//!      a. Runs ECS systems at the right stage (PreUpdate / FixedUpdate / Update / PostUpdate).
//!      b. Syncs physics (Transform <-> rapier).
//!      c. Updates the editor camera from the editor state's orbit angles.
//!      d. Snapshots the world for the UI.
//!      e. Runs the egui pass (draw_editor).
//!      f. Applies editor commands (add/delete entity, add/remove component, set transform, save/load scene).
//!      g. Renders the frame (scene to offscreen + egui on top).
//!      h. Presents.

use anyhow::Result;
use blaze_assets::SharedAssetRegistry;
use blaze_components::{
    Camera, DirectionalLight, Material, Mesh, MeshPrimitive, Name, PointLight, Sprite,
};
use blaze_core::{App, AppBuilder, Resources};
use blaze_ecs::{SharedSystems, SharedWorld, Stage};
use blaze_input::{Input, MouseButton};
use blaze_math::{Transform, Vec3};
use blaze_physics::{physics_sync_system, Collider, RigidBody, SharedPhysics};
use blaze_render::Renderer;
use blaze_scene::Scene;
use blaze_ui::{draw_editor, snapshot_world, EditorPanels, EditorState, SharedEditorState};
use parking_lot::Mutex;
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton as WinitMouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::platform::modifier_supplement::KeyEventExtModifierSupplement;
use winit::window::{Window, WindowId};

pub const DEFAULT_TITLE: &str = "Blaze Engine";

pub fn run(builder: AppBuilder) -> Result<()> {
    env_logger::try_init().ok();
    let event_loop = EventLoop::new()?;
    let mut handler = Handler { app: Some(builder), state: None };
    event_loop.run_app(&mut handler)?;
    Ok(())
}

struct LoopState {
    app: App,
    renderer: Renderer,
    egui_ctx: egui::Context,
    egui_winit: egui_winit::State,
    editor_state: SharedEditorState,
    panels: Arc<Mutex<EditorPanels>>,
    scene_texture_id: Option<egui::TextureId>,
    window: Arc<Window>,
}

struct Handler {
    app: Option<AppBuilder>,
    state: Option<LoopState>,
}

impl ApplicationHandler for Handler {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() { return; }
        let mut builder = self.app.take().expect("app builder missing");

        let window_attrs = Window::default_attributes()
            .with_title(DEFAULT_TITLE)
            .with_inner_size(winit::dpi::LogicalSize::new(1600, 900));
        let window = Arc::new(event_loop.create_window(window_attrs).expect("create_window"));
        let mut renderer = Renderer::new(window.clone()).expect("renderer init");

        let egui_ctx = egui::Context::default();
        let viewport_id = egui::ViewportId::ROOT;
        let egui_winit = egui_winit::State::new(
            egui_ctx.clone(),
            viewport_id,
            window.as_ref(),
            None,
            None,
            None,
        );

        let editor_state = Arc::new(Mutex::new(EditorState::default()));
        let panels = Arc::new(Mutex::new(EditorPanels::new()));

        builder.insert_resource(editor_state.clone());
        builder.insert_resource(panels.clone());

        let app = builder.build().expect("app build");

        // Register the offscreen scene texture with egui.
        let tex_id = renderer.register_scene_texture(&egui_ctx);
        editor_state.lock().scene_texture_id = Some(tex_id);

        self.state = Some(LoopState {
            app,
            renderer,
            egui_ctx,
            egui_winit,
            editor_state,
            panels,
            scene_texture_id: Some(tex_id),
            window,
        });
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(state) = self.state.as_mut() else { return };
        let _ = state.egui_winit.on_window_event(&state.window, &event);

        match event {
            WindowEvent::CloseRequested => { event_loop.exit(); }
            WindowEvent::Resized(size) => {
                state.renderer.resize(size.width, size.height);
                // Re-register the scene texture after resize.
                if state.scene_texture_id.is_some() {
                    let new_id = state.renderer.reregister_scene_texture(&state.egui_ctx);
                    state.scene_texture_id = Some(new_id);
                    state.editor_state.lock().scene_texture_id = Some(new_id);
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let Some(input) = state.app.resources.with::<Input, _, _>(|i| i.clone()) {
                    if let Some(key) = map_key(event.key_without_modifiers()) {
                        input.process_key(key, event.state == ElementState::Pressed);
                    }
                    if event.state == ElementState::Pressed
                        && event.key_without_modifiers() == winit::keyboard::Key::Named(winit::keyboard::NamedKey::Escape) {
                        event_loop.exit();
                    }
                }
            }
            WindowEvent::MouseInput { state: btn_state, button, .. } => {
                if let Some(input) = state.app.resources.with::<Input, _, _>(|i| i.clone()) {
                    if let Some(b) = map_button(button) {
                        input.process_button(b, btn_state == ElementState::Pressed);
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if let Some(input) = state.app.resources.with::<Input, _, _>(|i| i.clone()) {
                    input.process_mouse_move(blaze_math::Vec2::new(position.x as f32, position.y as f32));
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if let Some(input) = state.app.resources.with::<Input, _, _>(|i| i.clone()) {
                    let d = match delta {
                        winit::event::MouseScrollDelta::LineDelta(x, y) => blaze_math::Vec2::new(x, y),
                        winit::event::MouseScrollDelta::PixelDelta(p) => blaze_math::Vec2::new(p.x as f32, p.y as f32),
                    };
                    input.process_mouse_wheel(d);
                }
            }
            WindowEvent::RedrawRequested => {
                state.app.time.tick();

                // ----- 1. Run ECS systems -----
                let steps = state.app.time.drain_fixed_steps();
                let _ = state.app.resources.with::<SharedSystems, _, _>(|systems| {
                    let _ = state.app.resources.with::<SharedWorld, _, _>(|world| {
                        // Physics sync at fixed step.
                        let physics = state.app.resources.with::<SharedPhysics, _, _>(|p| p.clone());
                        if let Some(physics) = physics {
                            for _ in 0..steps {
                                let dt = state.app.time.fixed_step_secs();
                                {
                                    let mut w = world.lock();
                                    systems.lock().run_stage(Stage::PreUpdate, &mut w);
                                    physics_sync_system(&mut w, &physics, dt);
                                    systems.lock().run_stage(Stage::FixedUpdate, &mut w);
                                }
                            }
                        }
                        let mut w = world.lock();
                        systems.lock().run_stage(Stage::Update, &mut w);
                        systems.lock().run_stage(Stage::PostUpdate, &mut w);
                    });
                });

                // ----- 2. Update the editor camera from the editor state -----
                update_editor_camera(&state.app.resources, &state.editor_state);

                // ----- 3. Snapshot the world for the UI -----
                let snapshots = state.app.resources.with::<SharedWorld, _, _>(|world| {
                    snapshot_world(&*world.lock())
                }).unwrap_or_default();

                // ----- 4. Update FPS in editor state -----
                {
                    let mut es = state.editor_state.lock();
                    es.fps = state.app.time.fps();
                }

                // ----- 5. Drive the egui pass -----
                let mut raw_input: egui::RawInput = state.egui_winit.take_egui_input(&state.window);
                let screen_size = state.renderer.surface_config.clone();
                let pixels_per_point = state.window.scale_factor() as f32;
                raw_input.screen_rect = Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(screen_size.width as f32, screen_size.height as f32) / pixels_per_point,
                ));

                let assets = state.app.resources.with::<SharedAssetRegistry, _, _>(|a| a.clone());
                let panels_lock = state.panels.lock();
                let panels_ref: &EditorPanels = &*panels_lock;
                let editor_output = state.egui_ctx.run(raw_input, |ctx| {
                    let mut es = state.editor_state.lock();
                    let assets_ref = assets.as_ref();
                    draw_editor(ctx, &mut es, &snapshots, assets_ref);
                });
                drop(panels_lock);

                // ----- 6. Apply editor commands -----
                let pending_cmds: Vec<String> = {
                    let mut es = state.editor_state.lock();
                    let cmds: Vec<String> = es.console.iter()
                        .filter(|l| l.starts_with("__BLAZE_"))
                        .cloned()
                        .collect();
                    es.console.retain(|l| !l.starts_with("__BLAZE_"));
                    cmds
                };
                apply_editor_commands(&pending_cmds, &state.app.resources, &state.editor_state);

                // ----- 7. Handle save/load scene requests -----
                let (save_path, load_path) = {
                    let es = state.editor_state.lock();
                    (es.pending_save.clone(), es.pending_load.clone())
                };
                if let Some(path) = save_path {
                    let _ = state.app.resources.with::<SharedWorld, _, _>(|world| {
                        let scene = Scene::from_world(&*world.lock());
                        match scene.save_to_path(std::path::Path::new(&path)) {
                            Ok(_) => state.editor_state.lock().log(format!("Scene saved to {path}")),
                            Err(e) => state.editor_state.lock().log(format!("Save failed: {e}")),
                        }
                    });
                    state.editor_state.lock().pending_save = None;
                }
                if let Some(path) = load_path {
                    let _ = state.app.resources.with::<SharedWorld, _, _>(|world| {
                        match Scene::load_from_path(std::path::Path::new(&path)) {
                            Ok(scene) => {
                                let mut w = world.lock();
                                // Clear and respawn.
                                let to_despawn: Vec<_> = w.iter().map(|r| r.entity()).collect();
                                for e in to_despawn { let _ = w.despawn(e); }
                                let _ = scene.spawn_into(&mut w);
                                state.editor_state.lock().log(format!("Scene loaded from {path}"));
                            }
                            Err(e) => state.editor_state.lock().log(format!("Load failed: {e}")),
                        }
                    });
                    state.editor_state.lock().pending_load = None;
                }

                // ----- 8. Render -----
                let paint_jobs = state.egui_ctx.tessellate(editor_output.shapes, pixels_per_point);
                let screen_descriptor = egui_wgpu::ScreenDescriptor {
                    size_in_pixels: [screen_size.width, screen_size.height],
                    pixels_per_point,
                };
                let tex_id = state.scene_texture_id.unwrap_or(egui::TextureId::default());
                let world_arc = state.app.resources.with::<SharedWorld, _, _>(|w| w.clone());
                if let Some(world_arc) = world_arc {
                    let world = world_arc.lock();
                    if let Err(e) = state.renderer.render_frame(
                        &world,
                        &paint_jobs,
                        &screen_descriptor,
                        &editor_output.textures_delta,
                        tex_id,
                    ) {
                        log::error!("Render error: {e}");
                    }
                }

                // ----- 9. End-of-frame input flush -----
                if let Some(input) = state.app.resources.with::<Input, _, _>(|i| i.clone()) {
                    input.end_frame();
                }

                state.egui_winit.handle_platform_output(
                    &state.window,
                    editor_output.platform_output,
                );

                state.window.request_redraw();
            }
            _ => {}
        }
    }
}

/// Update the editor camera entity (the first entity with a Camera + Transform
/// that is the "game" camera — for simplicity, we treat the first camera found
/// as the editor camera and orbit it around the editor state's target).
fn update_editor_camera(resources: &Resources, editor_state: &SharedEditorState) {
    let world_arc = resources.with::<SharedWorld, _, _>(|w| w.clone());
    let Some(world_arc) = world_arc else { return; };
    let mut world = world_arc.lock();

    let es = editor_state.lock();
    let yaw = es.cam_orbit_yaw;
    let pitch = es.cam_orbit_pitch;
    let dist = es.cam_distance;
    let target = es.cam_target;
    drop(es);

    // Find the first entity with a Camera + Transform, then mutate that
    // Transform via query_mut (avoids the immutable EntityRef borrow issue).
    let cam_entity = world.query::<(&Camera, &Transform)>().iter().next().map(|(e, _)| e);
    if let Some(cam_entity) = cam_entity {
        // Spherical -> cartesian (orbit camera).
        let x = target.x + dist * pitch.cos() * yaw.sin();
        let y = target.y + dist * pitch.sin();
        let z = target.z + dist * pitch.cos() * yaw.cos();
        let pos = Vec3::new(x, y, z);
        // Build a look-at view matrix and extract the rotation as a quat.
        let view = glam::Mat4::look_at_rh(pos, target, Vec3::Y);
        let view_rot = glam::Quat::from_mat4(&view);
        let cam_rot = view_rot.conjugate();

        for (entity, (_, t)) in world.query_mut::<(&Camera, &mut Transform)>() {
            if entity == cam_entity {
                t.translation = pos;
                t.rotation = cam_rot;
                break;
            }
        }
    }
}

fn apply_editor_commands(cmds: &[String], resources: &Resources, editor_state: &SharedEditorState) {
    if cmds.is_empty() { return; }
    let world_arc = resources.with::<SharedWorld, _, _>(|w| w.clone());
    let Some(world_arc) = world_arc else { return; };
    let mut world = world_arc.lock();

    for cmd in cmds {
        if cmd == "__BLAZE_ADD_ENTITY__" {
            let count = world.iter().count();
            let e = world.spawn((Name(format!("Entity {count}")), Transform::default()));
            editor_state.lock().log(format!("Spawned entity {}", e.id()));
            editor_state.lock().selected_entity = Some(e.id() as u64);
        } else if cmd == "__BLAZE_NEW_SCENE__" {
            // Clear world.
            let to_despawn: Vec<_> = world.iter().map(|r| r.entity()).collect();
            for e in to_despawn { let _ = world.despawn(e); }
            editor_state.lock().log("New scene (world cleared)");
        } else if let Some(rest) = cmd.strip_prefix("__BLAZE_DEL_ENTITY__") {
            if let Ok(id_u64) = rest.parse::<u64>() {
                let to_delete = {
                    let mut iter = world.iter();
                    iter.find(|r| r.entity().id() as u64 == id_u64).map(|r| r.entity())
                };
                if let Some(e) = to_delete {
                    let _ = world.despawn(e);
                    editor_state.lock().log(format!("Deleted entity {id_u64}"));
                    let mut es = editor_state.lock();
                    if es.selected_entity == Some(id_u64) {
                        es.selected_entity = None;
                    }
                }
            }
        } else if let Some(rest) = cmd.strip_prefix("__BLAZE_ADD_TRANSFORM__") {
            if let Ok(id_u64) = rest.parse::<u64>() {
                let e = find_entity(&world, id_u64);
                if let Some(e) = e {
                    let _ = world.insert_one(e, Transform::default());
                    editor_state.lock().log(format!("Added Transform to entity {id_u64}"));
                }
            }
        } else if let Some(rest) = cmd.strip_prefix("__BLAZE_ADD_MESH__") {
            if let Ok(id_u64) = rest.parse::<u64>() {
                let e = find_entity(&world, id_u64);
                if let Some(e) = e {
                    let _ = world.insert_one(e, Mesh { primitive: MeshPrimitive::Cube });
                    let _ = world.insert_one(e, Material::default());
                    editor_state.lock().log(format!("Added Mesh+Material to entity {id_u64}"));
                }
            }
        } else if let Some(rest) = cmd.strip_prefix("__BLAZE_ADD_MATERIAL__") {
            if let Ok(id_u64) = rest.parse::<u64>() {
                let e = find_entity(&world, id_u64);
                if let Some(e) = e {
                    let _ = world.insert_one(e, Material::default());
                    editor_state.lock().log(format!("Added Material to entity {id_u64}"));
                }
            }
        } else if let Some(rest) = cmd.strip_prefix("__BLAZE_ADD_SPRITE__") {
            if let Ok(id_u64) = rest.parse::<u64>() {
                let e = find_entity(&world, id_u64);
                if let Some(e) = e {
                    let _ = world.insert_one(e, Sprite::default());
                    editor_state.lock().log(format!("Added Sprite to entity {id_u64}"));
                }
            }
        } else if let Some(rest) = cmd.strip_prefix("__BLAZE_ADD_CAMERA__") {
            if let Ok(id_u64) = rest.parse::<u64>() {
                let e = find_entity(&world, id_u64);
                if let Some(e) = e {
                    let _ = world.insert_one(e, Camera::default());
                    editor_state.lock().log(format!("Added Camera to entity {id_u64}"));
                }
            }
        } else if let Some(rest) = cmd.strip_prefix("__BLAZE_ADD_DIR_LIGHT__") {
            if let Ok(id_u64) = rest.parse::<u64>() {
                let e = find_entity(&world, id_u64);
                if let Some(e) = e {
                    let _ = world.insert_one(e, DirectionalLight::default());
                    editor_state.lock().log(format!("Added DirectionalLight to entity {id_u64}"));
                }
            }
        } else if let Some(rest) = cmd.strip_prefix("__BLAZE_ADD_POINT_LIGHT__") {
            if let Ok(id_u64) = rest.parse::<u64>() {
                let e = find_entity(&world, id_u64);
                if let Some(e) = e {
                    let _ = world.insert_one(e, PointLight::default());
                    editor_state.lock().log(format!("Added PointLight to entity {id_u64}"));
                }
            }
        } else if let Some(rest) = cmd.strip_prefix("__BLAZE_SET_TRANSFORM__") {
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if parts.len() == 7 {
                if let (Ok(id_u64), Ok(tx), Ok(ty), Ok(tz), Ok(sx), Ok(sy), Ok(sz)) = (
                    parts[0].parse::<u64>(),
                    parts[1].parse::<f32>(), parts[2].parse::<f32>(), parts[3].parse::<f32>(),
                    parts[4].parse::<f32>(), parts[5].parse::<f32>(), parts[6].parse::<f32>(),
                ) {
                    for (entity, t) in world.query_mut::<&mut Transform>() {
                        if entity.id() as u64 == id_u64 {
                            t.translation = Vec3::new(tx, ty, tz);
                            t.scale = Vec3::new(sx, sy, sz);
                            break;
                        }
                    }
                }
            }
        } else if let Some(rest) = cmd.strip_prefix("__BLAZE_ADD_RIGIDBODY__") {
            if let Ok(id_u64) = rest.parse::<u64>() {
                let e = find_entity(&world, id_u64);
                if let Some(e) = e {
                    let _ = world.insert_one(e, RigidBody::default());
                    let _ = world.insert_one(e, Collider::Box { half_extents: Vec3::new(0.5, 0.5, 0.5) });
                    editor_state.lock().log(format!("Added RigidBody+Collider to entity {id_u64}"));
                }
            }
        }
    }
}

fn find_entity(world: &blaze_ecs::World, id_u64: u64) -> Option<blaze_ecs::Entity> {
    let mut iter = world.iter();
    iter.find(|r| r.entity().id() as u64 == id_u64).map(|r| r.entity())
}

fn map_key(key: winit::keyboard::Key<winit::keyboard::SmolStr>) -> Option<blaze_input::Key> {
    use blaze_input::Key as BKey;
    match key {
        winit::keyboard::Key::Character(ref c) => match c.to_lowercase().as_str() {
            "a" => Some(BKey::A), "b" => Some(BKey::B), "c" => Some(BKey::C),
            "d" => Some(BKey::D), "e" => Some(BKey::E), "f" => Some(BKey::F),
            "g" => Some(BKey::G), "h" => Some(BKey::H), "i" => Some(BKey::I),
            "j" => Some(BKey::J), "k" => Some(BKey::K), "l" => Some(BKey::L),
            "m" => Some(BKey::M), "n" => Some(BKey::N), "o" => Some(BKey::O),
            "p" => Some(BKey::P), "q" => Some(BKey::Q), "r" => Some(BKey::R),
            "s" => Some(BKey::S), "t" => Some(BKey::T), "u" => Some(BKey::U),
            "v" => Some(BKey::V), "w" => Some(BKey::W), "x" => Some(BKey::X),
            "y" => Some(BKey::Y), "z" => Some(BKey::Z),
            "0" => Some(BKey::Num0), "1" => Some(BKey::Num1), "2" => Some(BKey::Num2),
            "3" => Some(BKey::Num3), "4" => Some(BKey::Num4), "5" => Some(BKey::Num5),
            "6" => Some(BKey::Num6), "7" => Some(BKey::Num7), "8" => Some(BKey::Num8),
            "9" => Some(BKey::Num9),
            _ => None,
        },
        winit::keyboard::Key::Named(named) => match named {
            winit::keyboard::NamedKey::Space => Some(BKey::Space),
            winit::keyboard::NamedKey::Enter => Some(BKey::Enter),
            winit::keyboard::NamedKey::Escape => Some(BKey::Escape),
            winit::keyboard::NamedKey::Tab => Some(BKey::Tab),
            winit::keyboard::NamedKey::Backspace => Some(BKey::Backspace),
            winit::keyboard::NamedKey::Delete => Some(BKey::Delete),
            winit::keyboard::NamedKey::Insert => Some(BKey::Insert),
            winit::keyboard::NamedKey::Home => Some(BKey::Home),
            winit::keyboard::NamedKey::End => Some(BKey::End),
            winit::keyboard::NamedKey::PageUp => Some(BKey::PageUp),
            winit::keyboard::NamedKey::PageDown => Some(BKey::PageDown),
            winit::keyboard::NamedKey::ArrowUp => Some(BKey::Up),
            winit::keyboard::NamedKey::ArrowDown => Some(BKey::Down),
            winit::keyboard::NamedKey::ArrowLeft => Some(BKey::Left),
            winit::keyboard::NamedKey::ArrowRight => Some(BKey::Right),
            winit::keyboard::NamedKey::Shift => Some(BKey::LeftShift),
            winit::keyboard::NamedKey::Control => Some(BKey::LeftCtrl),
            winit::keyboard::NamedKey::Alt => Some(BKey::LeftAlt),
            _ => None,
        },
        _ => None,
    }
}

fn map_button(b: WinitMouseButton) -> Option<MouseButton> {
    match b {
        WinitMouseButton::Left => Some(MouseButton::Left),
        WinitMouseButton::Right => Some(MouseButton::Right),
        WinitMouseButton::Middle => Some(MouseButton::Middle),
        WinitMouseButton::Back | WinitMouseButton::Forward | WinitMouseButton::Other(_) => None,
    }
}

// Silence unused-import warning for `color` (kept for future use).
#[allow(unused_imports)]
use blaze_math::color as _color_module;
