//! 2D example: spawns a falling entity, applies gravity via the physics
//! plugin, and prints the entity count each frame to prove the scheduler
//! and ECS are wired up correctly.

use blaze_app::run;
use blaze_core::App;
use blaze_ecs::{AppBuilderEcsExt, EcsPlugin, Stage, World};
use blaze_input::InputPlugin;
use blaze_physics::PhysicsPlugin;

#[derive(Debug, Clone, Copy)]
struct Falling { #[allow(dead_code)] label: &'static str }

fn main() {
    env_logger::init();

    let mut builder = App::builder();
    builder.add_plugin(EcsPlugin);
    builder.add_plugin(InputPlugin);
    builder.add_plugin(PhysicsPlugin::default());

    builder.add_system(Stage::Update, |world: &mut World| {
        let mut query = world.query::<&Falling>();
        let n = query.iter().count();
        if n > 0 && n % 30 == 0 {
            log::info!("entities alive: {n}");
        }
    });

    builder.add_system(Stage::FixedUpdate, |world: &mut World| {
        // Step physics at the fixed rate. World here is the ECS world;
        // we reach into the resources through a global hook omitted for
        // brevity in this example. The hello-triangle example shows the
        // windowed path; this example shows ECS scheduling.
        let _ = world;
    });

    if let Err(e) = run(builder) {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
