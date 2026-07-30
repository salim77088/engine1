//! Blaze Engine — Editor UI
//!
//! Full editor layout:
//!   * Top menu bar (File/Edit/View/Help)
//!   * Left Hierarchy panel (live entity list, add/delete, rename)
//!   * Central Viewport (displays the offscreen scene texture, camera controls)
//!   * Right Inspector (per-entity component editing, add/remove components)
//!   * Bottom Console (log feed + editor commands)
//!   * Floating Asset Browser
//!
//! The editor communicates with the runner via "sentinel" log lines
//! (`__BLAZE_*`) that the runner parses and applies to the world.

pub use egui;

use blaze_assets::SharedAssetRegistry;
use blaze_ecs::World;
use blaze_math::{Transform, Vec3};
use glam::EulerRot;
use parking_lot::Mutex;
use std::sync::Arc;

// ---------- user-registered custom panels ----------

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
}

pub type SharedEditorPanels = Arc<Mutex<EditorPanels>>;

// ---------- editor state ----------

/// Live editor state.
#[derive(Debug, Clone)]
pub struct EditorState {
    pub selected_entity: Option<u64>,
    pub console: Vec<String>,
    pub show_hierarchy: bool,
    pub show_inspector: bool,
    pub show_console: bool,
    pub show_assets: bool,
    pub show_about: bool,
    pub fps: f32,
    pub scene_texture_id: Option<egui::TextureId>,
    /// Pending scene file path to save/load.
    pub pending_save: Option<String>,
    pub pending_load: Option<String>,
    /// Editor camera orbit state.
    pub cam_orbit_yaw: f32,
    pub cam_orbit_pitch: f32,
    pub cam_distance: f32,
    pub cam_target: Vec3,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            selected_entity: None,
            console: vec![
                "Blaze Engine v0.3.0 editor initialised.".into(),
                "Tip: click +Add Entity, then add Mesh + Material components in the Inspector.".into(),
            ],
            show_hierarchy: true,
            show_inspector: true,
            show_console: true,
            show_assets: true,
            show_about: false,
            fps: 0.0,
            scene_texture_id: None,
            pending_save: None,
            pending_load: None,
            cam_orbit_yaw: 0.6,
            cam_orbit_pitch: 0.4,
            cam_distance: 8.0,
            cam_target: Vec3::ZERO,
        }
    }
}

impl EditorState {
    pub fn log(&mut self, line: impl Into<String>) {
        self.console.push(line.into());
        if self.console.len() > 500 {
            let drop_n = self.console.len() - 500;
            self.console.drain(0..drop_n);
        }
    }
}

pub type SharedEditorState = Arc<Mutex<EditorState>>;

// ---------- entity snapshot ----------

#[derive(Debug, Clone)]
pub struct EntitySnapshot {
    pub id: u64,
    pub name: String,
    pub transform: Option<Transform>,
    pub has_mesh: bool,
    pub has_sprite: bool,
    pub has_camera: bool,
    pub has_light: bool,
    pub has_rigidbody: bool,
}

pub fn snapshot_world(world: &World) -> Vec<EntitySnapshot> {
    use blaze_components::*;
    let mut out = Vec::new();
    let ids: Vec<blaze_ecs::Entity> = world.iter().map(|r| r.entity()).collect();
    for entity in ids {
        let id = entity.id() as u64;
        let name = world.entity(entity)
            .ok()
            .and_then(|r| r.get::<&Name>().map(|g| g.0.clone()))
            .unwrap_or_else(|| format!("Entity {}", id));
        let transform = world.entity(entity)
            .ok()
            .and_then(|r| r.get::<&Transform>().map(|g| *g));
        out.push(EntitySnapshot {
            id,
            name,
            transform,
            has_mesh: world.entity(entity).map(|r| r.get::<&Mesh>().is_some()).unwrap_or(false),
            has_sprite: world.entity(entity).map(|r| r.get::<&Sprite>().is_some()).unwrap_or(false),
            has_camera: world.entity(entity).map(|r| r.get::<&Camera>().is_some()).unwrap_or(false),
            has_light: world.entity(entity).map(|r| {
                r.get::<&DirectionalLight>().is_some() || r.get::<&PointLight>().is_some()
            }).unwrap_or(false),
            has_rigidbody: world.entity(entity).map(|r| {
                r.get::<&blaze_physics_bridge::RigidBody>().is_some()
            }).unwrap_or(false),
        });
    }
    out.sort_by_key(|e| e.id);
    out
}

/// Internal module so we can reference `blaze_physics::RigidBody` without
/// making blaze-ui depend on blaze-physics (avoids a circular dep).
mod blaze_physics_bridge {
    /// Stub type so `snapshot_world` compiles even when blaze-physics isn't
    /// a dependency. The real RigidBody type lives in blaze-physics.
    #[derive(Debug, Clone, Copy)]
    pub struct RigidBody;
}

// ---------- editor layout ----------

pub fn draw_editor(
    ctx: &egui::Context,
    state: &mut EditorState,
    snapshots: &[EntitySnapshot],
    assets: Option<&SharedAssetRegistry>,
) {
    // ----- top menu bar -----
    egui::TopBottomPanel::top("blaze_menu_bar").show(ctx, |ui| {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("New Scene").clicked() {
                    state.log("__BLAZE_NEW_SCENE__");
                    ui.close_menu();
                }
                if ui.button("Open Scene…").clicked() {
                    state.pending_load = Some("assets/scenes/main.scene.ron".into());
                    state.log("File > Open Scene (load requested)");
                    ui.close_menu();
                }
                if ui.button("Save Scene").clicked() {
                    state.pending_save = Some("assets/scenes/main.scene.ron".into());
                    state.log("File > Save Scene (save requested)");
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Exit").clicked() {
                    ui.close_menu();
                    std::process::exit(0);
                }
            });
            ui.menu_button("Edit", |ui| {
                if ui.button("Undo  (Ctrl+Z)").clicked() { ui.close_menu(); }
                if ui.button("Redo  (Ctrl+Y)").clicked() { ui.close_menu(); }
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
            ui.separator();
            ui.label(format!("Entities: {}", snapshots.len()));
        });
    });

    // ----- bottom console -----
    if state.show_console {
        egui::TopBottomPanel::bottom("blaze_console")
            .resizable(true)
            .default_height(140.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("Console");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Clear").clicked() {
                            state.console.clear();
                        }
                    });
                });
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for line in &state.console {
                        if line.starts_with("__BLAZE_") {
                            continue; // hide sentinel commands
                        }
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
                ui.horizontal(|ui| {
                    if ui.button("+ Add Entity").clicked() {
                        state.log("__BLAZE_ADD_ENTITY__");
                    }
                    if ui.button("- Delete").clicked() {
                        if let Some(id) = state.selected_entity {
                            state.log(format!("__BLAZE_DEL_ENTITY__{id}"));
                        }
                    }
                });
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for snap in snapshots {
                        let selected = state.selected_entity == Some(snap.id);
                        let icon = if snap.has_camera { "📷" }
                                   else if snap.has_light { "💡" }
                                   else if snap.has_mesh { "🧊" }
                                   else if snap.has_sprite { "🖼" }
                                   else { "⚪" };
                        let label = format!("{icon} {}", snap.name);
                        if ui.selectable_label(selected, label).clicked() {
                            state.selected_entity = Some(snap.id);
                        }
                    }
                });
            });
    }

    // ----- right inspector -----
    if state.show_inspector {
        egui::SidePanel::right("blaze_inspector")
            .resizable(true)
            .default_width(300.0)
            .show(ctx, |ui| {
                ui.heading("Inspector");
                ui.separator();
                let Some(selected_id) = state.selected_entity else {
                    ui.label("(nothing selected)");
                    ui.label("Click an entity in the Hierarchy.");
                    return;
                };
                let Some(snap) = snapshots.iter().find(|s| s.id == selected_id) else {
                    ui.label("(entity no longer exists)");
                    return;
                };
                ui.label(format!("Entity: {}", snap.name));
                ui.label(format!("ID:     {}", snap.id));
                ui.separator();

                // Transform section.
                ui.heading("Transform");
                if let Some(t) = snap.transform.as_ref() {
                    let mut t = *t;
                    let mut changed = false;
                    ui.horizontal(|ui| {
                        ui.label("Position");
                        changed |= ui.add(egui::DragValue::new(&mut t.translation.x).speed(0.05).prefix("X: ")).changed();
                        changed |= ui.add(egui::DragValue::new(&mut t.translation.y).speed(0.05).prefix("Y: ")).changed();
                        changed |= ui.add(egui::DragValue::new(&mut t.translation.z).speed(0.05).prefix("Z: ")).changed();
                    });
                    ui.horizontal(|ui| {
                        ui.label("Scale   ");
                        changed |= ui.add(egui::DragValue::new(&mut t.scale.x).speed(0.05).range(0.01..=100.0).prefix("X: ")).changed();
                        changed |= ui.add(egui::DragValue::new(&mut t.scale.y).speed(0.05).range(0.01..=100.0).prefix("Y: ")).changed();
                        changed |= ui.add(egui::DragValue::new(&mut t.scale.z).speed(0.05).range(0.01..=100.0).prefix("Z: ")).changed();
                    });
                    let (yaw, pitch, roll) = t.rotation.to_euler(EulerRot::YXZ);
                    let mut yaw = yaw.to_degrees();
                    let mut pitch = pitch.to_degrees();
                    let mut roll = roll.to_degrees();
                    ui.horizontal(|ui| {
                        ui.label("Rotation");
                        let c = ui.add(egui::DragValue::new(&mut yaw).speed(1.0).prefix("Y: ").suffix("°")).changed();
                        let c2 = ui.add(egui::DragValue::new(&mut pitch).speed(1.0).prefix("P: ").suffix("°")).changed();
                        let c3 = ui.add(egui::DragValue::new(&mut roll).speed(1.0).prefix("R: ").suffix("°")).changed();
                        if c || c2 || c3 {
                            t.rotation = glam::Quat::from_euler(
                                EulerRot::YXZ,
                                yaw.to_radians(),
                                pitch.to_radians(),
                                roll.to_radians(),
                            );
                            changed = true;
                        }
                    });
                    if changed {
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

                ui.separator();

                // Components section.
                ui.heading("Components");
                ui.horizontal_wrapped(|ui| {
                    if !snap.has_mesh {
                        if ui.button("+ Mesh").clicked() {
                            state.log(format!("__BLAZE_ADD_MESH__{}", snap.id));
                        }
                    }
                    if snap.has_mesh && !snap_transform_has(snap, "Material") {
                        if ui.button("+ Material").clicked() {
                            state.log(format!("__BLAZE_ADD_MATERIAL__{}", snap.id));
                        }
                    }
                    if !snap.has_sprite {
                        if ui.button("+ Sprite").clicked() {
                            state.log(format!("__BLAZE_ADD_SPRITE__{}", snap.id));
                        }
                    }
                    if !snap.has_camera {
                        if ui.button("+ Camera").clicked() {
                            state.log(format!("__BLAZE_ADD_CAMERA__{}", snap.id));
                        }
                    }
                    if !snap.has_light {
                        if ui.button("+ Directional Light").clicked() {
                            state.log(format!("__BLAZE_ADD_DIR_LIGHT__{}", snap.id));
                        }
                        if ui.button("+ Point Light").clicked() {
                            state.log(format!("__BLAZE_ADD_POINT_LIGHT__{}", snap.id));
                        }
                    }
                });

                // Component flags (read-only summary).
                ui.separator();
                ui.label("Components on this entity:");
                ui.horizontal_wrapped(|ui| {
                    let mut tags = Vec::new();
                    if snap.has_mesh { tags.push("Mesh"); }
                    if snap_transform_has(snap, "Material") { tags.push("Material"); }
                    if snap.has_sprite { tags.push("Sprite"); }
                    if snap.has_camera { tags.push("Camera"); }
                    if snap.has_light { tags.push("Light"); }
                    if snap.has_rigidbody { tags.push("RigidBody"); }
                    if tags.is_empty() {
                        ui.label("(none)");
                    } else {
                        for t in tags {
                            ui.label(format!("• {t}"));
                        }
                    }
                });
            });
    }

    // ----- central viewport -----
    egui::CentralPanel::default().show(ctx, |ui| {
        // Display the scene texture if registered.
        if let Some(tex_id) = state.scene_texture_id {
            let avail = ui.available_size();
            let (rect, _) = ui.allocate_exact_size(avail, egui::Sense::drag());
            // Draw the scene texture as a full-panel image.
            ui.painter().image(
                tex_id,
                rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
            // Viewport overlay text.
            ui.painter().text(
                rect.min + egui::vec2(8.0, 8.0),
                egui::Align2::LEFT_TOP,
                format!(
                    "Camera: yaw {:5.1}° pitch {:5.1}° dist {:4.1}\nTarget: ({:5.2}, {:5.2}, {:5.2})\nDrag to orbit · scroll to zoom",
                    state.cam_orbit_yaw.to_degrees(),
                    state.cam_orbit_pitch.to_degrees(),
                    state.cam_distance,
                    state.cam_target.x, state.cam_target.y, state.cam_target.z,
                ),
                egui::FontId::proportional(12.0),
                egui::Color32::WHITE,
            );

            // Camera controls: drag to orbit, scroll to zoom.
            let drag = ui.interact(rect, ui.id().with("viewport_drag"), egui::Sense::drag());
            if drag.dragged() {
                let d = drag.drag_delta();
                state.cam_orbit_yaw -= d.x * 0.01;
                state.cam_orbit_pitch += d.y * 0.01;
                state.cam_orbit_pitch = state.cam_orbit_pitch.clamp(-1.4, 1.4);
            }
            let scroll_delta = ctx.input(|i| i.smooth_scroll_delta);
            state.cam_distance = (state.cam_distance - scroll_delta.y * 0.5).max(1.0).min(100.0);
        } else {
            ui.label("(scene texture not registered yet)");
        }
    });

    // ----- floating asset browser -----
    if state.show_assets {
        egui::Window::new("Asset Browser")
            .default_pos([16.0, 360.0])
            .default_size([280.0, 240.0])
            .resizable(true)
            .show(ctx, |ui| {
                if let Some(reg) = assets {
                    let reg = reg.read();
                    let files = reg.list_files();
                    if files.is_empty() {
                        ui.label("(no assets — drop files under assets/)");
                    } else {
                        for f in &files {
                            ui.horizontal(|ui| {
                                ui.label("📁");
                                ui.label(f);
                            });
                        }
                    }
                } else {
                    ui.label("(asset registry not available)");
                }
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
                    ui.label("v0.3.0 — alpha");
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
}

fn snap_transform_has(_snap: &EntitySnapshot, _name: &str) -> bool {
    // The EntitySnapshot doesn't track every component. For now, we
    // return true for Material if Mesh is present (common case).
    // A future version will track every component on the snapshot.
    _snap.has_mesh
}
