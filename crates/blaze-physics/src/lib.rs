//! Blaze Engine — Physics
//!
//! Wraps [`rapier3d`] behind a `Physics` facade. Components:
//!   * `RigidBody` — marks an entity as a dynamic/static/kinematic body.
//!   * `Collider`  — collision shape (box, sphere, capsule).
//!
//! The `physics_sync_system` runs at the `FixedUpdate` stage: it pushes
//! `Transform` changes into rapier, steps the simulation, then pulls the
//! new positions back into the ECS `Transform` components.

use blaze_core::{AppBuilder, Plugin};
use blaze_ecs::{World};
use blaze_math::{Transform, Vec3};
use parking_lot::Mutex;
use rapier3d::dynamics::{
    CCDSolver, ImpulseJointSet, IntegrationParameters, IslandManager,
    MultibodyJointSet, RigidBodyBuilder, RigidBodySet, RigidBodyType,
};
use rapier3d::geometry::{BroadPhaseMultiSap, ColliderBuilder, ColliderSet, NarrowPhase, SharedShape};
use rapier3d::math::Vector;
use rapier3d::pipeline::PhysicsPipeline;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Serializable body-kind enum (mirrors rapier's RigidBodyType but with
/// serde support).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum BodyKind {
    Dynamic,
    Fixed,
    KinematicPositionBased,
    KinematicVelocityBased,
}

impl Default for BodyKind {
    fn default() -> Self { Self::Dynamic }
}

impl BodyKind {
    fn to_rapier(self) -> RigidBodyType {
        match self {
            BodyKind::Dynamic => RigidBodyType::Dynamic,
            BodyKind::Fixed => RigidBodyType::Fixed,
            BodyKind::KinematicPositionBased => RigidBodyType::KinematicPositionBased,
            BodyKind::KinematicVelocityBased => RigidBodyType::KinematicVelocityBased,
        }
    }
}

/// Component: marks an entity as a physics body.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RigidBody {
    pub kind: BodyKind,
    pub linear_damping: f32,
    pub angular_damping: f32,
}

impl Default for RigidBody {
    fn default() -> Self {
        Self {
            kind: BodyKind::Dynamic,
            linear_damping: 0.1,
            angular_damping: 0.1,
        }
    }
}

/// Component: collision shape attached to an entity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Collider {
    Box { half_extents: Vec3 },
    Sphere { radius: f32 },
    Capsule { half_height: f32, radius: f32 },
}

impl Default for Collider {
    fn default() -> Self {
        Self::Box { half_extents: Vec3::new(0.5, 0.5, 0.5) }
    }
}

/// Internal bridge between ECS entity id and rapier's RigidBodyHandle.
#[derive(Debug, Clone, Copy)]
pub struct PhysicsLink {
    pub entity_id: u32,
    pub body_handle: rapier3d::dynamics::RigidBodyHandle,
    pub collider_handle: rapier3d::geometry::ColliderHandle,
}

/// 3D vector alias used by rapier.
pub type Vec3F = Vector<f32>;

/// State bundle for a 3D rapier world. Stored as `Arc<Mutex<Physics>>`.
pub struct Physics {
    pub pipeline: PhysicsPipeline,
    pub islands: IslandManager,
    pub broad_phase: BroadPhaseMultiSap,
    pub narrow_phase: NarrowPhase,
    pub bodies: RigidBodySet,
    pub colliders: ColliderSet,
    pub impulse_joints: ImpulseJointSet,
    pub multibody_joints: MultibodyJointSet,
    pub ccd_solver: CCDSolver,
    pub gravity: Vec3F,
    pub integration: IntegrationParameters,
}

impl Default for Physics {
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
            gravity: Vector::new(0.0, -9.81, 0.0),
            integration: IntegrationParameters::default(),
        }
    }
}

impl Physics {
    pub fn new() -> Self { Self::default() }

    pub fn with_gravity(gravity: Vec3F) -> Self {
        Self { gravity, ..Default::default() }
    }

    /// Step the simulation by `dt` seconds.
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
pub type SharedPhysics = Arc<Mutex<Physics>>;

pub struct PhysicsPlugin {
    pub gravity: Vec3F,
}

impl Default for PhysicsPlugin {
    fn default() -> Self {
        Self { gravity: Vector::new(0.0, -9.81, 0.0) }
    }
}

impl Plugin for PhysicsPlugin {
    fn name(&self) -> &str { "blaze-physics" }

    fn build(&self, app: &mut AppBuilder) {
        let physics = Physics::with_gravity(self.gravity);
        app.insert_resource(Arc::new(Mutex::new(physics)) as SharedPhysics);
    }
}

/// Convert a glam Vec3 to a rapier translation vector.
fn to_rapier_vec3(v: Vec3) -> Vector<f32> {
    Vector::new(v.x, v.y, v.z)
}

/// Convert a glam Quat to a rapier axis-angle Vector3.
fn to_rapier_axis_angle(q: glam::Quat) -> Vector<f32> {
    let (axis, angle) = if q.w.abs() >= 1.0 {
        // Identity rotation.
        (Vector::new(1.0, 0.0, 0.0), 0.0)
    } else {
        let angle = 2.0 * q.w.acos();
        let s = (1.0 - q.w * q.w).sqrt();
        let axis = if s < 1e-4 {
            Vector::new(1.0, 0.0, 0.0)
        } else {
            Vector::new(q.x / s, q.y / s, q.z / s)
        };
        (axis, angle)
    };
    axis * angle
}

/// Convert a rapier translation vector to glam Vec3.
fn from_rapier_vec3(v: &Vector<f32>) -> Vec3 {
    Vec3::new(v.x, v.y, v.z)
}

/// Convert a rapier quaternion to glam Quat.
fn from_rapier_quat(q: &rapier3d::math::Rotation<f32>) -> glam::Quat {
    let q = q.as_ref();
    glam::Quat::from_xyzw(q.i, q.j, q.k, q.w)
}

/// Sync system that should be registered at `Stage::FixedUpdate`:
///   1. For every entity with `(RigidBody, Collider, Transform)` and no
///      `PhysicsLink`, create the rapier body + collider and store the link.
///   2. Push `Transform` changes (only for kinematic bodies) into rapier.
///   3. Step the simulation.
///   4. Pull rapier positions back into `Transform` (only for dynamic bodies).
pub fn physics_sync_system(world: &mut World, physics: &SharedPhysics, dt: f32) {
    let mut physics = physics.lock();

    // 1. Ensure every RigidBody entity has a PhysicsLink.
    // First collect entities that need a physics body (immutable borrow).
    let mut to_create: Vec<(blaze_ecs::Entity, RigidBody, Collider, Transform)> = Vec::new();
    {
        // Query RigidBody + Collider + Transform immutably first.
        let mut q = world.query::<(&RigidBody, &Collider, &Transform)>();
        let linked: std::collections::HashSet<u32> = world.query::<&PhysicsLink>()
            .iter()
            .map(|(e, _)| e.id())
            .collect();
        for (entity, (rb, collider, transform)) in q.iter() {
            if !linked.contains(&entity.id()) {
                to_create.push((entity, *rb, collider.clone(), *transform));
            }
        }
    }
    for (entity, rb, collider, transform) in to_create {
        let rapier_kind = rb.kind.to_rapier();
        let pos = rapier3d::math::Isometry::new(
            to_rapier_vec3(transform.translation),
            to_rapier_axis_angle(transform.rotation),
        );
        let body_builder = RigidBodyBuilder::new(rapier_kind)
            .position(pos)
            .linear_damping(rb.linear_damping)
            .angular_damping(rb.angular_damping);
        let body_handle = physics.bodies.insert(body_builder);
        let shape: SharedShape = match &collider {
            Collider::Box { half_extents } => SharedShape::cuboid(half_extents.x, half_extents.y, half_extents.z),
            Collider::Sphere { radius } => SharedShape::ball(*radius),
            Collider::Capsule { half_height, radius } => SharedShape::capsule_y(*half_height, *radius),
        };
        let collider_builder = ColliderBuilder::new(shape);
        // Split the Physics into two mutable refs to its fields.
        // SAFETY: we have exclusive access via the MutexGuard, and the two
        // fields don't overlap.
        let Physics { colliders, bodies, .. } = &mut *physics;
        let collider_handle = colliders.insert_with_parent(collider_builder, body_handle, bodies);
        let _ = world.insert_one(entity, PhysicsLink {
            entity_id: entity.id(),
            body_handle,
            collider_handle,
        });
    }

    // 2. Push kinematic Transform changes into rapier.
    for (entity, (link, transform)) in world.query_mut::<(&PhysicsLink, &Transform)>() {
        let _ = entity;
        if let Some(body) = physics.bodies.get_mut(link.body_handle) {
            if body.body_type() != RigidBodyType::Dynamic {
                let pos = rapier3d::math::Isometry::new(
                    to_rapier_vec3(transform.translation),
                    to_rapier_axis_angle(transform.rotation),
                );
                body.set_next_kinematic_position(pos);
            }
        }
    }

    // 3. Step.
    physics.step(dt);

    // 4. Pull dynamic body positions back into Transform.
    for (_, (link, transform)) in world.query_mut::<(&PhysicsLink, &mut Transform)>() {
        if let Some(body) = physics.bodies.get(link.body_handle) {
            if body.body_type() == RigidBodyType::Dynamic {
                transform.translation = from_rapier_vec3(body.translation());
                transform.rotation = from_rapier_quat(body.rotation());
            }
        }
    }
}
