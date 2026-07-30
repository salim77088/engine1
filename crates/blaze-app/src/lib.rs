//! Blaze Engine — App integration
//!
//! Wires the core/ecs/input/render/physics/ui/script crates into a single
//! default winit-based runner. The runner opens a window, drives the main
//! loop, runs the editor UI on top of the game viewport via `egui_wgpu`,
//! and forwards editor commands (add entity, delete entity, set transform)
//! back into the ECS world.

use anyhow::Result;
use blaze_core::{App, AppBuilder};
use blaze_ecs::{SharedSystems, SharedWorld, Stage};
use blaze_input::{Input, MouseButton};
use blaze_math::Transform;
use blaze_render::Renderer;
use blaze_ui::{draw_editor, snapshot_world, EditorPanels, EditorState, SharedEditorState};
use parking_lot::Mutex;
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton as WinitMouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::platform::modifier_supplement::KeyEventExtModifierSupplement;
use winit::window::{Window, WindowId};

/// Default Blaze window title.
pub const DEFAULT_TITLE: &str = "Blaze Engine";

/// Builder-friendly entry point that constructs an `EventLoop`, builds the
/// `App`, and runs the loop. This is the function most games will call from
/// their `main`.
pub fn run(builder: AppBuilder) -> Result<()> {
    env_logger::try_init().ok();
    let event_loop = EventLoop::new()?;
    let mut handler = Handler { app: Some(builder), state: None };
    event_loop.run_app(&mut handler)?;
    Ok(())
}

/// Internal state held across the winit event loop.
struct LoopState {
    app: App,
    renderer: Renderer,
    /// egui state (context + winit integration).
    egui_ctx: egui::Context,
    egui_winit: egui_winit::State,
    /// Editor state (selection, console, panel visibility).
    editor_state: SharedEditorState,
    /// User-registered custom panels.
    panels: Arc<Mutex<EditorPanels>>,
    window: Arc<Window>,
}

/// The winit `ApplicationHandler` we drive from [`run`].
struct Handler {
    app: Option<AppBuilder>,
    state: Option<LoopState>,
}

impl ApplicationHandler for Handler {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() { return; }
        let builder = self.app.take().expect("app builder missing");

        // Create the window first so the renderer can target it.
        let window_attrs = Window::default_attributes()
            .with_title(DEFAULT_TITLE)
            .with_inner_size(winit::dpi::LogicalSize::new(1280, 720));
        let window = Arc::new(event_loop.create_window(window_attrs).expect("create_window"));

        let renderer = Renderer::new(window.clone()).expect("renderer init");

        // egui context + winit state.
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

        // Editor state (live selection, console, etc.)
        let editor_state = Arc::new(Mutex::new(EditorState::default()));

        // Insert the editor state as a resource so user code can read & mutate it.
        let panels = Arc::new(Mutex::new(EditorPanels::new()));
        let mut builder = builder;
        builder.insert_resource(editor_state.clone());
        builder.insert_resource(panels.clone());

        let app = builder.build().expect("app build");

        self.state = Some(LoopState {
            app,
            renderer,
            egui_ctx,
            egui_winit,
            editor_state,
            panels,
            window,
        });
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(state) = self.state.as_mut() else { return };

        // Forward every event to egui-winit first so the editor UI gets
        // keyboard/mouse focus before the game.
        let _egui_response = state.egui_winit.on_window_event(&state.window, &event);

        match event {
            WindowEvent::CloseRequested => { event_loop.exit(); }
            WindowEvent::Resized(size) => {
                state.renderer.resize(size.width, size.height);
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

                // Run ECS systems.
                let steps = state.app.time.drain_fixed_steps();
                let _ = state.app.resources.with::<SharedSystems, _, _>(|systems| {
                    let _ = state.app.resources.with::<SharedWorld, _, _>(|world| {
                        for _ in 0..steps {
                            systems.lock().run_stage(Stage::PreUpdate, &mut world.lock());
                            systems.lock().run_stage(Stage::FixedUpdate, &mut world.lock());
                        }
                        systems.lock().run_stage(Stage::Update, &mut world.lock());
                        systems.lock().run_stage(Stage::PostUpdate, &mut world.lock());
                    });
                });

                // Snapshot the world so the editor UI can read entity data
                // without holding the world lock during the egui pass.
                let snapshots = state.app.resources.with::<SharedWorld, _, _>(|world| {
                    snapshot_world(&*world.lock())
                }).unwrap_or_default();

                // Update FPS in editor state.
                {
                    let mut es = state.editor_state.lock();
                    es.fps = state.app.time.fps();
                }

                // Drive the egui pass.
                let mut raw_input: egui::RawInput = state.egui_winit.take_egui_input(&state.window);
                let screen_size = state.renderer.surface_config.clone();
                let pixels_per_point = state.window.scale_factor() as f32;
                raw_input.screen_rect = Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(screen_size.width as f32, screen_size.height as f32) / pixels_per_point,
                ));

                let panels_clone_lock = state.panels.lock();
                let panels_clone: &EditorPanels = &*panels_clone_lock;
                let editor_output = state.egui_ctx.run(raw_input, |ctx| {
                    let mut es = state.editor_state.lock();
                    draw_editor(ctx, &mut es, &snapshots, panels_clone);
                });
                drop(panels_clone_lock);

                // Apply editor commands (sentinels the UI wrote into the console).
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

                // Hand the paint jobs + textures to the renderer.
                let paint_jobs = state.egui_ctx.tessellate(editor_output.shapes, pixels_per_point);
                let screen_descriptor = egui_wgpu::ScreenDescriptor {
                    size_in_pixels: [screen_size.width, screen_size.height],
                    pixels_per_point: pixels_per_point,
                };
                if let Err(e) = state.renderer.render_with_ui(
                    &paint_jobs,
                    &screen_descriptor,
                    &editor_output.textures_delta,
                ) {
                    log::error!("Render error: {e}");
                }

                // End-of-frame input flush.
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

/// Translate the sentinel strings the editor UI writes into the console into
/// actual mutations of the ECS world.
fn apply_editor_commands(cmds: &[String], resources: &blaze_core::Resources, editor_state: &SharedEditorState) {
    if cmds.is_empty() { return; }
    let world_arc = resources.with::<SharedWorld, _, _>(|w| w.clone());
    let Some(world_arc) = world_arc else { return; };
    let mut world = world_arc.lock();

    for cmd in cmds {
        if cmd == "__BLAZE_ADD_ENTITY__" {
            let entity = world.spawn((Transform::default(),));
            editor_state.lock().log(format!("Spawned entity {}", entity.id()));
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
                let e = {
                    let mut iter = world.iter();
                    iter.find(|r| r.entity().id() as u64 == id_u64).map(|r| r.entity())
                };
                if let Some(e) = e {
                    let _ = world.insert_one(e, Transform::default());
                    editor_state.lock().log(format!("Added Transform to entity {id_u64}"));
                }
            }
        } else if let Some(rest) = cmd.strip_prefix("__BLAZE_SET_TRANSFORM__") {
            // Format: {id} tx ty tz sx sy sz
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if parts.len() == 7 {
                if let (Ok(id_u64), Ok(tx), Ok(ty), Ok(tz), Ok(sx), Ok(sy), Ok(sz)) = (
                    parts[0].parse::<u64>(),
                    parts[1].parse::<f32>(), parts[2].parse::<f32>(), parts[3].parse::<f32>(),
                    parts[4].parse::<f32>(), parts[5].parse::<f32>(), parts[6].parse::<f32>(),
                ) {
                    // query_mut gives us mutable access to all Transforms; we
                    // locate the matching entity id and apply the new values.
                    let target_id = id_u64;
                    for (entity, t) in world.query_mut::<&mut Transform>() {
                        if entity.id() as u64 == target_id {
                            t.translation = blaze_math::Vec3::new(tx, ty, tz);
                            t.scale = blaze_math::Vec3::new(sx, sy, sz);
                            break;
                        }
                    }
                }
            }
        }
    }
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
