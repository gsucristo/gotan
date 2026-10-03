//! Engine-agnostic physics API. Gameplay code only touches the types here; the
//! engine behind them is a swappable backend (currently Avian, `avian.rs`).
//!
//! A backend must:
//! - give `StaticBody` / `KinematicBody` entities colliders of their `size`;
//! - in `MotionSystems::Move`, move each kinematic body by its `Velocity` with
//!   collide-and-slide, honoring one-way platforms, then write back the
//!   post-slide velocity and `Grounded`;
//! - smooth body `Transform`s between fixed ticks (render interpolation);
//! - draw collider debug shapes while `PhysicsDebug` is on.

mod avian;

use bevy::prelude::*;

pub const PHYSICS_TICK_HZ: f64 = 240.0;

pub const GRAVITY: f32 = 2500.0;
pub const MAX_FALL_SPEED: f32 = 1600.0;

/// Fixed-tick motion pipeline, in order.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum MotionSystems {
    /// Controllers (player input, AI) set `Velocity`.
    Intent,
    /// Gravity is added to every kinematic body.
    Gravity,
    /// The backend moves bodies and resolves collisions.
    Move,
}

/// Immovable box collider, centered on the entity's `Transform`.
#[derive(Component, Debug, Clone, Copy)]
#[require(Transform)]
pub struct StaticBody {
    pub size: Vec2,
    /// Solid only to bodies landing on it from above.
    pub one_way: bool,
}

/// Box collider moved by its `Velocity` (characters). Centered on the entity's
/// `Transform`, never rotated.
#[derive(Component, Debug, Clone, Copy)]
#[require(Transform, Velocity, Grounded)]
pub struct KinematicBody {
    pub size: Vec2,
}

/// World units/s. Controllers write the desired velocity; after `Move` it holds
/// what's left after sliding along whatever was hit.
#[derive(Component, Debug, Default, Clone, Copy, Deref, DerefMut)]
pub struct Velocity(pub Vec2);

/// Whether the body stood on something during the last tick.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq, Deref)]
pub struct Grounded(pub bool);

/// Toggles backend collider debug drawing.
#[derive(Resource, Debug, Default)]
pub struct PhysicsDebug(pub bool);

pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Time::<Fixed>::from_hz(PHYSICS_TICK_HZ))
            .init_resource::<PhysicsDebug>()
            .configure_sets(
                FixedUpdate,
                (
                    MotionSystems::Intent,
                    MotionSystems::Gravity,
                    MotionSystems::Move,
                )
                    .chain(),
            )
            .add_systems(FixedUpdate, apply_gravity.in_set(MotionSystems::Gravity))
            .add_plugins(avian::AvianBackendPlugin);
    }
}

fn apply_gravity(time: Res<Time>, mut bodies: Query<&mut Velocity, With<KinematicBody>>) {
    let dt = time.delta_secs();
    for mut v in &mut bodies {
        v.y = (v.y - GRAVITY * dt).max(-MAX_FALL_SPEED);
    }
}
