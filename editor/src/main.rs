//! Blaze Engine — Editor entry point
//!
//! Builds the default Blaze app, registers all built-in plugins, and seeds
//! a starter 3D scene so the viewport shows something useful on first launch:
//!   * An editor camera (orbiting around the origin)
//!   * A directional light (the "sun")
//!   * A floor plane
//!   * A demo cube with a PBR material
//!   * A demo point light

use blaze_app::run;
use blaze_components::{
    Camera, DirectionalLight, Material, Mesh, MeshPrimitive, Name, PointLight,
};
use blaze_core::App;
use blaze_ecs::{AppBuilderEcsExt, EcsPlugin, Stage, World};
use blaze_input::InputPlugin;
use blaze_math::{color, Vec3};
use blaze_math::Transform;
use blaze_physics::PhysicsPlugin;
use blaze_script::ScriptPlugin;
use blaze_assets::AssetPlugin;

fn main() {
    env_logger::init();

    let mut builder = App::builder();
    builder.add_plugin(EcsPlugin);
    builder.add_plugin(InputPlugin);
    builder.add_plugin(PhysicsPlugin::default());
    builder.add_plugin(ScriptPlugin);
    builder.add_plugin(AssetPlugin::default());

    // Seed a starter scene exactly once.
    builder.add_system(Stage::PostUpdate, |world: &mut World| {
        let count = world.iter().count();
        if count > 0 { return; }
        // Editor camera.
        world.spawn((
            Name("Editor Camera".into()),
            Transform::from_translation(Vec3::new(4.0, 3.0, 4.0)),
            Camera::default(),
        ));
        // Sun light.
        world.spawn((
            Name("Sun".into()),
            Transform::default(),
            DirectionalLight {
                color: color::WHITE,
                intensity: 2.0,
                direction: Vec3::new(-0.4, -1.0, -0.3),
            },
        ));
        // Floor.
        world.spawn((
            Name("Floor".into()),
            Transform { translation: Vec3::new(0.0, -1.0, 0.0), ..Default::default() },
            Mesh { primitive: MeshPrimitive::Plane },
            Material {
                base_color: color::rgb(80, 80, 90),
                roughness: 0.9,
                metallic: 0.0,
                emissive: color::rgb(0, 0, 0),
            },
        ));
        // Demo cube.
        world.spawn((
            Name("Cube".into()),
            Transform {
                translation: Vec3::new(0.0, 0.0, 0.0),
                ..Default::default()
            },
            Mesh { primitive: MeshPrimitive::Cube },
            Material {
                base_color: color::rgb(200, 80, 60),
                roughness: 0.4,
                metallic: 0.2,
                emissive: color::rgb(0, 0, 0),
            },
        ));
        // Demo sphere.
        world.spawn((
            Name("Sphere".into()),
            Transform {
                translation: Vec3::new(2.0, 0.0, 0.0),
                ..Default::default()
            },
            Mesh { primitive: MeshPrimitive::Sphere },
            Material {
                base_color: color::rgb(60, 120, 200),
                roughness: 0.2,
                metallic: 0.8,
                emissive: color::rgb(0, 0, 0),
            },
        ));
        // Point light.
        world.spawn((
            Name("Point Light".into()),
            Transform::from_translation(Vec3::new(-2.0, 2.0, 0.0)),
            PointLight {
                color: color::rgb(255, 200, 100),
                intensity: 8.0,
                range: 15.0,
            },
        ));
        log::info!("Seeded starter 3D scene: camera, sun, floor, cube, sphere, point light.");
    });

    log::info!("Blaze Engine editor starting…");
    if let Err(e) = run(builder) {
        log::error!("Blaze Engine exited with error: {e}");
        std::process::exit(1);
    }
}
