//! Blaze Engine — Editor entry point
//!
//! Builds a default Blaze app with all built-in plugins enabled and hands
//! it off to `blaze_app::run`, which opens a window, drives the main loop
//! and renders the editor UI.

use blaze_app::run;
use blaze_core::App;
use blaze_ecs::{AppBuilderEcsExt, EcsPlugin, Stage, World};
use blaze_input::InputPlugin;
use blaze_physics::PhysicsPlugin;
use blaze_script::ScriptPlugin;

fn main() {
    env_logger::init();

    let mut builder = App::builder();
    builder.add_plugin(EcsPlugin);
    builder.add_plugin(InputPlugin);
    builder.add_plugin(PhysicsPlugin::default());
    builder.add_plugin(ScriptPlugin);

    // Register a tiny demo system that logs FPS once per second-ish.
    builder.add_system(Stage::Update, |_world: &mut World| {
        // Intentionally minimal — the engine's job here is just to prove
        // the scheduler runs. Replace with your gameplay systems.
    });

    log::info!("Blaze Engine editor starting…");
    if let Err(e) = run(builder) {
        log::error!("Blaze Engine exited with error: {e}");
        std::process::exit(1);
    }
}
