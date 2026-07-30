//! Blaze Engine — Editor entry point
//!
//! Builds a default Blaze app with all built-in plugins enabled and seeds a
//! few demo entities so the hierarchy panel is not empty on first launch.

use blaze_app::run;
use blaze_core::App;
use blaze_ecs::{AppBuilderEcsExt, EcsPlugin, Stage, World};
use blaze_input::InputPlugin;
use blaze_math::{Transform, Vec3};
use blaze_physics::PhysicsPlugin;
use blaze_script::ScriptPlugin;

fn main() {
    env_logger::init();

    let mut builder = App::builder();
    builder.add_plugin(EcsPlugin);
    builder.add_plugin(InputPlugin);
    builder.add_plugin(PhysicsPlugin::default());
    builder.add_plugin(ScriptPlugin);

    // Seed a few demo entities so the hierarchy panel isn't empty on launch.
    builder.add_system(Stage::Update, |world: &mut World| {
        // This system runs every frame — we check the entity count and
        // spawn the seed entities exactly once.
        let count = world.query::<&Transform>().iter().count();
        if count == 0 {
            world.spawn((Transform::from_translation(Vec3::new(0.0, 0.0, 0.0)),));
            world.spawn((Transform::from_translation(Vec3::new(2.0, 0.0, 0.0)),));
            world.spawn((Transform::from_translation(Vec3::new(-2.0, 0.0, 0.0)),));
            log::info!("Seeded 3 demo entities.");
        }
    });

    log::info!("Blaze Engine editor starting…");
    if let Err(e) = run(builder) {
        log::error!("Blaze Engine exited with error: {e}");
        std::process::exit(1);
    }
}
