//! Blaze Engine — App integration
//!
//! Wires the core/ecs/input/render/physics/ui/script crates into a single
//! default winit-based runner. Users can either call `App::builder().run()`
//! after registering their own runner, or use [`DefaultRunner`] which opens
//! a window, drives the main loop, and integrates the editor UI.

use anyhow::Result;
use blaze_core::{App, AppBuilder};
use blaze_ecs::{SharedSystems, SharedWorld, Stage};
use blaze_input::{Input, MouseButton};
use blaze_math::Vec2;
use blaze_render::Renderer;
use blaze_ui::{EditorPanels, SharedEditorPanels};
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
    #[allow(dead_code)]
    panels: SharedEditorPanels,
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

        // Register default editor panels.
        let panels = Arc::new(Mutex::new(EditorPanels::new()));
        {
            let p = panels.clone();
            panels.lock().register(move |ctx| {
                let fps = 0.0; // Updated externally — placeholder.
                blaze_ui::stats_panel(ctx, fps);
                blaze_ui::hierarchy_panel(ctx);
                blaze_ui::inspector_panel(ctx);
            });
            let _ = p; // keep the clone referenced
        }

        // Insert the panels resource so the user can extend them.
        let mut builder = builder;
        builder.insert_resource(panels.clone());

        let app = builder.build().expect("app build");

        self.state = Some(LoopState { app, renderer, panels, window });
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(state) = self.state.as_mut() else { return };
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
                    input.process_mouse_move(Vec2::new(position.x as f32, position.y as f32));
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if let Some(input) = state.app.resources.with::<Input, _, _>(|i| i.clone()) {
                    let d = match delta {
                        winit::event::MouseScrollDelta::LineDelta(x, y) => Vec2::new(x, y),
                        winit::event::MouseScrollDelta::PixelDelta(p) => Vec2::new(p.x as f32, p.y as f32),
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

                // Render.
                if let Err(e) = state.renderer.render() {
                    log::error!("Render error: {e}");
                }

                // End-of-frame input flush.
                if let Some(input) = state.app.resources.with::<Input, _, _>(|i| i.clone()) {
                    input.end_frame();
                }

                state.window.request_redraw();
            }
            _ => {}
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
