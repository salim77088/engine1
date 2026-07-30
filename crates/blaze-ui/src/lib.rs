//! Blaze Engine — UI
//!
//! Re-exports [`egui`] and provides:
//!   * `EditorState` — the live state of the editor (selected entity, console
//!     buffer, asset list, panel visibility flags),
//!   * `EditorPanels` — a registry for user-supplied custom panels,
//!   * `draw_editor` — the function that draws the full Blaze editor layout
//!     every frame, reading from a snapshot of the ECS world so the
//!     hierarchy and inspector show live data.

pub use egui;

use blaze_ecs::World;
use blaze_math::Transform;
use parking_lot::Mutex;
use std::sync::Arc;

/// A panel the user can register with `EditorPanels::register`.
pub type PanelFn = dyn Fn(&egui::Context) + Send + Sync;

/// Registry of user-supplied custom panels (drawn after the built-in
/// editor layout).
pub struct EditorPanels {
    panels: Vec<Box<PanelFn>>,
}

impl Default for EditorPanels {
    fn default() -> Self { Self { panels: Vec::new() } }
}

impl EditorPanels {
    pub fn new() -> Self { Self::default() }

    pub fn register<F: Fn(&egui::Context) + Send + Sync + 'static>(&mut self, f: F) {
        self.panels.push(Box::new(f));
    }

    pub fn render(&self, ctx: &egui::Context) {
        for panel in &self.panels {
            panel(ctx);
        }
    }

    pub fn count(&self) -> usize { self.panels.len() }
}

/// Shareable handle.
pub type SharedEditorPanels = Arc<Mutex<EditorPanels>>;

/// Live editor state. Stored as `Arc<Mutex<EditorState>>` and updated by
/// both the editor UI (when the user clicks things) and the engine
/// (when systems run).
#[derive(Debug, Clone)]
pub struct EditorState {
    /// Currently selected entity id (as a u64, since hecs::Entity isn't
    /// trivially constructible from the UI).
    pub selected_entity: Option<u64>,
    /// Console log lines.
    pub console: Vec<String>,
    /// Whether each major panel is visible.
    pub show_hierarchy: bool,
    pub show_inspector: bool,
    pub show_console: bool,
    pub show_assets: bool,
    pub show_about: bool,
    /// Asset browser mock entries (in a real engine these would come
    /// from the asset system).
    pub assets: Vec<String>,
    /// FPS reported by the runner.
    pub fps: f32,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            selected_entity: None,
            console: vec![
                "Blaze Engine v0.1.0 editor initialised.".into(),
                "Tip: use the Hierarchy panel to add entities, then edit them in the Inspector.".into(),
            ],
            show_hierarchy: true,
            show_inspector: true,
            show_console: true,
            show_assets: true,
            show_about: false,
            assets: vec![
                "textures/".into(),
                "meshes/".into(),
                "scripts/".into(),
                "scenes/".into(),
            ],
            fps: 0.0,
        }
    }
}

impl EditorState {
    pub fn log(&mut self, line: impl Into<String>) {
        self.console.push(line.into());
        // Keep the console bounded.
        if self.console.len() > 500 {
            let drop_n = self.console.len() - 500;
            self.console.drain(0..drop_n);
        }
    }
}

/// Shareable handle.
pub type SharedEditorState = Arc<Mutex<EditorState>>;

/// Snapshot of an entity + its transform that the UI uses to render the
/// hierarchy and inspector without holding a lock on the world for too long.
#[derive(Debug, Clone)]
pub struct EntitySnapshot {
    pub id: u64,
    pub name: String,
    pub transform: Option<Transform>,
}

/// Snapshot the world's entities for the editor UI.
pub fn snapshot_world(world: &World) -> Vec<EntitySnapshot> {
    let mut out = Vec::new();
    for (entity, transform) in world.query::<Option<&Transform>>().iter() {
        let id = entity.id() as u64;
        let name = format!("Entity {}", id);
        out.push(EntitySnapshot {
            id,
            name,
            transform: transform.copied(),
        });
    }
    out.sort_by_key(|e| e.id);
    out
}

/// The full Blaze editor layout. Call this once per frame from the runner.
pub fn draw_editor(
    ctx: &egui::Context,
    state: &mut EditorState,
    snapshots: &[EntitySnapshot],
    panels: &EditorPanels,
) {
    // ----- top menu bar -----
    egui::TopBottomPanel::top("blaze_menu_bar").show(ctx, |ui| {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("New Scene").clicked() {
                    state.log("File > New Scene (no-op stub)");
                    ui.close_menu();
                }
                if ui.button("Open Scene…").clicked() {
                    state.log("File > Open Scene (no-op stub)");
                    ui.close_menu();
                }
                if ui.button("Save Scene").clicked() {
                    state.log("File > Save Scene (no-op stub)");
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Exit").clicked() {
                    ui.close_menu();
                    std::process::exit(0);
                }
            });
            ui.menu_button("Edit", |ui| {
                if ui.button("Undo").clicked() { ui.close_menu(); }
                if ui.button("Redo").clicked() { ui.close_menu(); }
            });
            ui.menu_button("View", |ui| {
                ui.checkbox(&mut state.show_hierarchy, "Hierarchy");
                ui.checkbox(&mut state.show_inspector, "Inspector");
                ui.checkbox(&mut state.show_console,  "Console");
                ui.checkbox(&mut state.show_assets,   "Asset Browser");
            });
            ui.menu_button("Help", |ui| {
                if ui.button("About Blaze").clicked() {
                    state.show_about = true;
                    ui.close_menu();
                }
            });
            ui.separator();
            ui.label(format!("FPS: {:5.1}", state.fps));
        });
    });

    // ----- bottom console -----
    if state.show_console {
        egui::TopBottomPanel::bottom("blaze_console")
            .resizable(true)
            .default_height(140.0)
            .show(ctx, |ui| {
                ui.heading("Console");
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for line in &state.console {
                        ui.label(line);
                    }
                });
            });
    }

    // ----- left hierarchy -----
    if state.show_hierarchy {
        egui::SidePanel::left("blaze_hierarchy")
            .resizable(true)
            .default_width(240.0)
            .show(ctx, |ui| {
                ui.heading("Hierarchy");
                ui.separator();
                ui.label(format!("{} entities", snapshots.len()));
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for snap in snapshots {
                        let selected = state.selected_entity == Some(snap.id);
                        if ui.selectable_label(selected, &snap.name).clicked() {
                            state.selected_entity = Some(snap.id);
                            state.log(format!("Selected {}", snap.name));
                        }
                    }
                });
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("+ Add Entity").clicked() {
                        state.log("Hierarchy: 'Add Entity' requested (runner will spawn one next frame)");
                        // The runner watches for this sentinel and creates a real entity.
                        state.log("__BLAZE_ADD_ENTITY__");
                    }
                    if ui.button("- Delete").clicked() {
                        if let Some(id) = state.selected_entity {
                            state.log(format!("Hierarchy: delete requested for entity {id}"));
                            state.log(format!("__BLAZE_DEL_ENTITY__{id}"));
                        }
                    }
                });
            });
    }

    // ----- right inspector -----
    if state.show_inspector {
        egui::SidePanel::right("blaze_inspector")
            .resizable(true)
            .default_width(280.0)
            .show(ctx, |ui| {
                ui.heading("Inspector");
                ui.separator();
                let Some(selected_id) = state.selected_entity else {
                    ui.label("(nothing selected)");
                    return;
                };
                let Some(snap) = snapshots.iter().find(|s| s.id == selected_id) else {
                    ui.label("(entity no longer exists)");
                    return;
                };
                ui.label(format!("Entity: {}", snap.name));
                ui.label(format!("ID:    {}", snap.id));
                ui.separator();
                if let Some(t) = snap.transform.as_ref() {
                    ui.heading("Transform");
                    let mut t = *t;
                    let mut changed = false;
                    ui.horizontal(|ui| {
                        ui.label("Position");
                        changed |= ui.add(egui::DragValue::new(&mut t.translation.x).speed(0.01).prefix("X: ")).changed();
                        changed |= ui.add(egui::DragValue::new(&mut t.translation.y).speed(0.01).prefix("Y: ")).changed();
                        changed |= ui.add(egui::DragValue::new(&mut t.translation.z).speed(0.01).prefix("Z: ")).changed();
                    });
                    ui.horizontal(|ui| {
                        ui.label("Scale   ");
                        changed |= ui.add(egui::DragValue::new(&mut t.scale.x).speed(0.01).range(0.01..=100.0).prefix("X: ")).changed();
                        changed |= ui.add(egui::DragValue::new(&mut t.scale.y).speed(0.01).range(0.01..=100.0).prefix("Y: ")).changed();
                        changed |= ui.add(egui::DragValue::new(&mut t.scale.z).speed(0.01).range(0.01..=100.0).prefix("Z: ")).changed();
                    });
                    ui.horizontal(|ui| {
                        ui.label("Rotation (yaw/pitch/roll degrees)");
                        let (yaw, pitch, roll) = t.rotation.to_euler(glam::EulerRot::YXZ);
                        ui.label(format!("Y:{:6.1}  P:{:6.1}  R:{:6.1}", yaw.to_degrees(), pitch.to_degrees(), roll.to_degrees()));
                    });
                    if changed {
                        // Use a simple parseable format: tx ty tz sx sy sz
                        state.log(format!(
                            "__BLAZE_SET_TRANSFORM__{} {} {} {} {} {} {}",
                            snap.id,
                            t.translation.x, t.translation.y, t.translation.z,
                            t.scale.x, t.scale.y, t.scale.z,
                        ));
                    }
                } else {
                    ui.label("No Transform component.");
                    if ui.button("+ Add Transform").clicked() {
                        state.log(format!("__BLAZE_ADD_TRANSFORM__{}", snap.id));
                    }
                }
            });
    }

    // ----- central viewport -----
    egui::CentralPanel::default().show(ctx, |ui| {
        ui.heading("Viewport");
        ui.separator();
        ui.label(
            "The game viewport renders here. By default Blaze shows the built-in \
             demo triangle so you can confirm the GPU pipeline is alive. Your \
             game systems populate this view as you add renderable components."
        );
        ui.add_space(8.0);
        ui.label(format!(
            "Window size: {:.0} x {:.0}  |  Selected: {}",
            ctx.screen_rect().width(),
            ctx.screen_rect().height(),
            state.selected_entity
                .map(|id| format!("entity {id}"))
                .unwrap_or_else(|| "—".into())
        ));
    });

    // ----- bottom-right floating asset browser -----
    if state.show_assets {
        egui::Window::new("Asset Browser")
            .default_pos([16.0, 320.0])
            .default_size([260.0, 220.0])
            .resizable(true)
            .show(ctx, |ui| {
                for a in &state.assets {
                    ui.horizontal(|ui| {
                        ui.label("📁");
                        ui.label(a);
                    });
                }
                ui.separator();
                ui.label("(stub — wire this to your asset system)");
            });
    }

    // ----- about modal -----
    if state.show_about {
        egui::Window::new("About Blaze Engine")
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.heading("Blaze Engine");
                    ui.label("v0.1.0 — alpha");
                    ui.add_space(8.0);
                    ui.label("A lightweight Rust game engine");
                    ui.label("built on wgpu, winit, rapier, hecs, egui, rhai.");
                    ui.add_space(8.0);
                    ui.label("MIT OR Apache-2.0");
                    ui.add_space(8.0);
                    if ui.button("Close").clicked() {
                        state.show_about = false;
                    }
                });
            });
    }

    // ----- user-registered custom panels -----
    panels.render(ctx);
}

// blaze_math::Vec3 is already re-exported via blaze_math in downstream code;
