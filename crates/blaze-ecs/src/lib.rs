//! Blaze Engine — ECS
//!
//! Lightweight wrapper around [`hecs`] providing a `World` resource, a
//! `System` trait and a stage-based scheduler that runs systems in the
//! `Update` and `FixedUpdate` phases of the engine loop.

use blaze_core::{AppBuilder, Plugin};
pub use hecs::{Component, Entity, Query, QueryBorrow, QueryOne, Ref, World};
use parking_lot::Mutex;
use std::sync::Arc;

/// Type stored as a resource: an `Arc<Mutex<World>>` so it can be safely
/// shared between the runner thread and the editor UI thread.
pub type SharedWorld = Arc<Mutex<World>>;

/// Type stored as a resource: an `Arc<Mutex<Systems>>` so that `add_system`
/// can mutate the scheduler during build and the runner can iterate it.
pub type SharedSystems = Arc<Mutex<Systems>>;

/// Plugin that registers an empty [`World`] resource and the [`Systems`]
/// scheduler used to drive per-frame logic.
pub struct EcsPlugin;

impl Plugin for EcsPlugin {
    fn name(&self) -> &str { "blaze-ecs" }

    fn build(&self, app: &mut AppBuilder) {
        app.insert_resource(Arc::new(Mutex::new(Systems::default())) as SharedSystems);
        app.insert_resource(Arc::new(Mutex::new(World::new())) as SharedWorld);
    }
}

/// A system is a function that runs every frame (or every fixed step).
pub type SystemFn = dyn Fn(&mut World) + Send + Sync;

/// Stage at which a system should run.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Stage {
    PreUpdate,
    Update,
    FixedUpdate,
    PostUpdate,
}

/// A boxed system plus its target stage.
struct SystemEntry {
    stage: Stage,
    system: Box<SystemFn>,
}

/// Collection of systems scheduled on stages. Stored inside an
/// `Arc<Mutex<Systems>>` resource so that the `add_system` API can mutate it
/// during build and the runner can iterate it during the loop.
#[derive(Default)]
pub struct Systems {
    entries: Vec<SystemEntry>,
}

impl Systems {
    pub fn add<S: Fn(&mut World) + Send + Sync + 'static>(&mut self, stage: Stage, system: S) {
        self.entries.push(SystemEntry { stage, system: Box::new(system) });
    }

    pub fn run_stage(&self, stage: Stage, world: &mut World) {
        for entry in &self.entries {
            if entry.stage == stage {
                (entry.system)(world);
            }
        }
    }

    pub fn count(&self) -> usize { self.entries.len() }
}

/// Extension trait so users can write `app.add_system(Stage::Update, my_system)`.
pub trait AppBuilderEcsExt {
    fn add_system<S: Fn(&mut World) + Send + Sync + 'static>(&mut self, stage: Stage, system: S) -> &mut Self;
    fn world(&self) -> Option<World>;
}

impl AppBuilderEcsExt for AppBuilder {
    fn add_system<S: Fn(&mut World) + Send + Sync + 'static>(&mut self, stage: Stage, system: S) -> &mut Self {
        // Pull the Arc clone out, mutate, done.
        let arc = self.resources.with::<SharedSystems, _, _>(|s| s.clone());
        if let Some(s) = arc {
            s.lock().add(stage, system);
        } else {
            log::warn!("add_system called before EcsPlugin; system dropped.");
        }
        self
    }

    fn world(&self) -> Option<World> {
        // World is not Clone, so we cannot retrieve a copy from Resources.
        None
    }
}
