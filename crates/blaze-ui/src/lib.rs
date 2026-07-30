//! Blaze Engine — UI
//!
//! Re-exports [`egui`] and provides an `EditorPanels` helper that draws the
//! default Blaze editor layout (main menu bar + scene hierarchy + inspector +
//! viewport stats). The actual wgpu integration of egui is wired up inside
//! `blaze-app`'s `EditorRunner`, since that's where the renderer's device/queue
//! live.

pub use egui;

use parking_lot::Mutex;
use std::sync::Arc;

/// A panel the user can register with `EditorPanels::register`.
pub type PanelFn = dyn Fn(&egui::Context) + Send + Sync;

/// Default editor panel set.
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

/// Built-in panel that shows engine stats in the top bar.
pub fn stats_panel(ctx: &egui::Context, fps: f32) {
    egui::TopBottomPanel::top("blaze_top_bar").show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.label(format!("Blaze Engine  |  FPS: {:5.1}", fps));
            ui.separator();
            ui.menu_button("File", |ui| {
                if ui.button("New Scene").clicked() { ui.close_menu(); }
                if ui.button("Open Scene…").clicked() { ui.close_menu(); }
                if ui.button("Save Scene").clicked() { ui.close_menu(); }
                if ui.button("Exit").clicked() { ui.close_menu(); }
            });
            ui.menu_button("View", |ui| {
                if ui.button("Toggle Hierarchy").clicked() { ui.close_menu(); }
                if ui.button("Toggle Inspector").clicked() { ui.close_menu(); }
            });
            ui.menu_button("Help", |ui| {
                ui.label("Blaze Engine v0.1.0");
                ui.label("MIT / Apache-2.0 licensed");
            });
        });
    });
}

/// Built-in left-side scene hierarchy panel (placeholder content).
pub fn hierarchy_panel(ctx: &egui::Context) {
    egui::SidePanel::left("blaze_hierarchy").show(ctx, |ui| {
        ui.heading("Scene Hierarchy");
        ui.separator();
        ui.label("(no entities yet)");
    });
}

/// Built-in right-side inspector panel (placeholder content).
pub fn inspector_panel(ctx: &egui::Context) {
    egui::SidePanel::right("blaze_inspector").show(ctx, |ui| {
        ui.heading("Inspector");
        ui.separator();
        ui.label("(nothing selected)");
    });
}
