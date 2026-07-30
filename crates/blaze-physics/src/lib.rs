//! Blaze Engine — Physics
//!
//! Wraps [`rapier2d`] behind a small `Physics2D` facade with a `gravity`
//! field and a `step` method driven by the engine's fixed-step accumulator.
//! Components are intentionally minimal — the engine integrates tightly
//! with the ECS layer so users can drop a `RigidBody` component on any
//! entity and have it simulated.

use blaze_core::{AppBuilder, Plugin};
use parking_lot::Mutex;
use rapier2d::dynamics::{CCDSolver, ImpulseJointSet, IntegrationParameters, IslandManager, MultibodyJointSet, RigidBodySet};
use rapier2d::geometry::{BroadPhaseMultiSap, ColliderSet, NarrowPhase};
use rapier2d::math::Vector;
use rapier2d::pipeline::PhysicsPipeline;
use std::sync::Arc;

/// 2D vector alias used by rapier.
pub type Vec2 = Vector<f32>;

/// State bundle for a 2D rapier world. Stored as `Arc<Mutex<Physics2D>>`
/// in the engine's resource table.
pub struct Physics2D {
    pub pipeline: PhysicsPipeline,
    pub islands: IslandManager,
    pub broad_phase: BroadPhaseMultiSap,
    pub narrow_phase: NarrowPhase,
    pub bodies: RigidBodySet,
    pub colliders: ColliderSet,
    pub impulse_joints: ImpulseJointSet,
    pub multibody_joints: MultibodyJointSet,
    pub ccd_solver: CCDSolver,
    pub gravity: Vec2,
    pub integration: IntegrationParameters,
}

impl Default for Physics2D {
    fn default() -> Self {
        Self {
            pipeline: PhysicsPipeline::new(),
            islands: IslandManager::new(),
            broad_phase: BroadPhaseMultiSap::new(),
            narrow_phase: NarrowPhase::new(),
            bodies: RigidBodySet::new(),
            colliders: ColliderSet::new(),
            impulse_joints: ImpulseJointSet::new(),
            multibody_joints: MultibodyJointSet::new(),
            ccd_solver: CCDSolver::new(),
            gravity: Vector::new(0.0, -9.81),
            integration: IntegrationParameters::default(),
        }
    }
}

impl Physics2D {
    pub fn new() -> Self { Self::default() }

    pub fn with_gravity(gravity: Vec2) -> Self {
        Self { gravity, ..Default::default() }
    }

    /// Advance the simulation by `dt` seconds.
    pub fn step(&mut self, dt: f32) {
        self.integration.dt = dt.max(1e-4);
        let physics_hooks = ();
        let event_handler = ();
        self.pipeline.step(
            &self.gravity,
            &self.integration,
            &mut self.islands,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.bodies,
            &mut self.colliders,
            &mut self.impulse_joints,
            &mut self.multibody_joints,
            &mut self.ccd_solver,
            None,
            &physics_hooks,
            &event_handler,
        );
    }
}

/// Shareable handle.
pub type SharedPhysics2D = Arc<Mutex<Physics2D>>;

pub struct PhysicsPlugin {
    pub gravity: Vec2,
}

impl Default for PhysicsPlugin {
    fn default() -> Self {
        Self { gravity: Vector::new(0.0, -9.81) }
    }
}

impl Plugin for PhysicsPlugin {
    fn name(&self) -> &str { "blaze-physics" }

    fn build(&self, app: &mut AppBuilder) {
        let physics = Physics2D::with_gravity(self.gravity);
        app.insert_resource(Arc::new(Mutex::new(physics)) as SharedPhysics2D);
    }
}
