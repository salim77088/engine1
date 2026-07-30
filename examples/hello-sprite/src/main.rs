//! Blaze 2D sprite example — spawns a 2D orthographic camera and three
//! colored sprites.

use blaze_app::run;
use blaze_components::{Camera, Name, Sprite};
use blaze_core::App;
use blaze_ecs::{AppBuilderEcsExt, EcsPlugin, Stage, World};
use blaze_input::InputPlugin;
use blaze_math::{color, Vec2, Vec3};
use blaze_math::Transform;
use blaze_physics::PhysicsPlugin;

fn main() {
    env_logger::init();

    let mut b = App::builder();
    b.add_plugin(EcsPlugin);
    b.add_plugin(InputPlugin);
    b.add_plugin(PhysicsPlugin::default());

    b.add_system(Stage::PostUpdate, |world: &mut World| {
        if world.iter().count() > 0 { return; }
        world.spawn((
            Name("2D Camera".into()),
            Transform::from_translation(Vec3::new(0.0, 0.0, 5.0)),
            Camera {
                fov_radians: 60.0f32.to_radians(),
                near: 0.1,
                far: 100.0,
                orthographic: true,
                ortho_size: 3.0,
                clear_color: color::rgb(30, 30, 40),
            },
        ));
        for (i, c) in [
            (color::rgb(220, 80, 80), -2.0),
            (color::rgb(80, 220, 80), 0.0),
            (color::rgb(80, 80, 220), 2.0),
        ].iter().enumerate() {
            world.spawn((
                Name(format!("Sprite {}", i)),
                Transform::from_translation(Vec3::new(c.1, 0.0, 0.0)),
                Sprite {
                    color: c.0,
                    size: Vec2::new(1.0, 1.0),
                    texture: None,
                },
            ));
        }
    });

    if let Err(e) = run(b) {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
