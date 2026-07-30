//! Smallest possible Blaze 3D example: spawns a camera, a light, and a cube,
//! then runs the editor-style app loop.

use blaze_app::run;
use blaze_components::{Camera, DirectionalLight, Material, Mesh, MeshPrimitive, Name};
use blaze_core::App;
use blaze_ecs::{AppBuilderEcsExt, EcsPlugin, Stage, World};
use blaze_input::InputPlugin;
use blaze_math::{color, Vec3};
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
            Name("Camera".into()),
            Transform::from_translation(Vec3::new(3.0, 3.0, 3.0)),
            Camera::default(),
        ));
        world.spawn((
            Name("Sun".into()),
            Transform::default(),
            DirectionalLight {
                color: color::WHITE,
                intensity: 2.0,
                direction: Vec3::new(-0.4, -1.0, -0.3),
            },
        ));
        world.spawn((
            Name("Cube".into()),
            Transform::default(),
            Mesh { primitive: MeshPrimitive::Cube },
            Material {
                base_color: color::rgb(200, 80, 60),
                roughness: 0.5,
                metallic: 0.2,
                emissive: color::rgb(0, 0, 0),
            },
        ));
    });

    if let Err(e) = run(b) {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
