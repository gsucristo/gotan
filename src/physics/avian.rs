//! Avian backend for the physics API (`super`). Everything Avian-specific lives
//! here; rolling our own physics means replacing this file.

use std::iter;

use avian2d::prelude::*;
use bevy::prelude::*;

use super::{GRAVITY, Grounded, KinematicBody, MotionSystems, PhysicsDebug, StaticBody, Velocity};

/// World units per meter: the character is ~100 units for ~1.87 m. Avian
/// scales its tolerances (e.g. move-and-slide skin width) by this.
const LENGTH_UNIT: f32 = 54.0;

/// Hit normals at least this upward (cos ~45°) count as standing on ground.
const GROUND_NORMAL_Y: f32 = 0.7;

/// How far below a body to look for ground (world units).
const GROUND_PROBE: f32 = 2.0;

/// Slack when deciding a body is above a one-way platform: a resting body
/// hovers a skin width over the top.
const ONE_WAY_SLACK: f32 = 1.0;

pub(super) struct AvianBackendPlugin;

impl Plugin for AvianBackendPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            PhysicsPlugins::default().with_length_unit(LENGTH_UNIT),
            PhysicsDebugPlugin,
        ))
        // Only dynamic bodies use Avian's gravity (none yet); keep it in step
        // with ours.
        .insert_resource(avian2d::prelude::Gravity(Vec2::NEG_Y * GRAVITY))
        .add_observer(add_static_collider)
        .add_observer(add_kinematic_collider)
        .add_systems(Startup, configure_gizmos)
        .add_systems(FixedUpdate, move_and_slide.in_set(MotionSystems::Move))
        .add_systems(Update, sync_debug.run_if(resource_changed::<PhysicsDebug>));
    }
}

fn add_static_collider(add: On<Add, StaticBody>, bodies: Query<&StaticBody>, mut commands: Commands) {
    let Ok(body) = bodies.get(add.entity) else {
        return;
    };
    commands
        .entity(add.entity)
        .insert((RigidBody::Static, Collider::rectangle(body.size.x, body.size.y)));
}

fn add_kinematic_collider(
    add: On<Add, KinematicBody>,
    bodies: Query<&KinematicBody>,
    mut commands: Commands,
) {
    let Ok(body) = bodies.get(add.entity) else {
        return;
    };
    commands.entity(add.entity).insert((
        RigidBody::Kinematic,
        Collider::rectangle(body.size.x, body.size.y),
        // Position changes only through move-and-slide, never Avian's integrator.
        CustomPositionIntegration,
        TransformInterpolation,
    ));
}

/// Reads and writes Avian's `Position`, not `Transform`: between ticks the
/// `Transform` holds the render-interpolated value, while a `Position` written
/// during the tick is authoritative and gets copied to `Transform` after the
/// physics step. `MoveAndSlide` itself reads every collider's `Position`, so
/// the write goes through a `ParamSet`.
fn move_and_slide(
    time: Res<Time>,
    mut physics: ParamSet<(MoveAndSlide, Query<&mut Position, With<KinematicBody>>)>,
    mut bodies: Query<(Entity, &KinematicBody, &Collider, &mut Velocity, &mut Grounded)>,
    statics: Query<(Entity, &StaticBody, &Transform)>,
) {
    for (entity, body, collider, mut vel, mut grounded) in &mut bodies {
        let mover = physics.p0();
        let Ok((_, start, ..)) = mover.colliders.get(entity) else {
            continue;
        };
        let start = start.0;

        // A one-way platform is passable unless the body is falling and was
        // above its top when the tick started.
        let bottom = start.y - body.size.y / 2.0;
        let passable = statics
            .iter()
            .filter(|(_, s, t)| {
                s.one_way && (vel.y > 0.0 || bottom < t.translation.y + s.size.y / 2.0 - ONE_WAY_SLACK)
            })
            .map(|(e, ..)| e);
        let filter = SpatialQueryFilter::from_excluded_entities(iter::once(entity).chain(passable));

        let config = MoveAndSlideConfig::default();
        let out = mover.move_and_slide(
            collider,
            start,
            0.0,
            vel.0,
            time.delta(),
            &config,
            &filter,
            |_| MoveAndSlideHitResponse::Accept,
        );
        let mut pos = out.position;
        let mut v = out.projected_velocity;

        // Ground probe. The slide only reports ground it was pushed into, which
        // a mostly-horizontal walking sweep misses, so look just below
        // explicitly — then snap onto it so the body doesn't bob in the skin gap.
        let skin = mover.length_unit.0 * config.skin_width;
        let ground = if v.y <= 0.0 {
            mover
                .cast_move(collider, pos, 0.0, Vec2::NEG_Y * GROUND_PROBE, skin, &filter)
                .filter(|hit| hit.normal1.y > GROUND_NORMAL_Y)
        } else {
            None
        };
        if let Some(hit) = ground {
            pos.y -= hit.distance;
            v.y = 0.0;
        }

        if let Ok(mut position) = physics.p1().get_mut(entity) {
            position.0 = pos;
        }
        vel.0 = v;
        grounded.set_if_neq(Grounded(ground.is_some()));
    }
}

fn configure_gizmos(mut store: ResMut<GizmoConfigStore>, debug: Res<PhysicsDebug>) {
    let (config, _) = store.config_mut::<PhysicsGizmos>();
    config.enabled = debug.0;
    // Colliders sit inside the platform meshes; draw on top.
    config.depth_bias = -1.0;
}

fn sync_debug(mut store: ResMut<GizmoConfigStore>, debug: Res<PhysicsDebug>) {
    store.config_mut::<PhysicsGizmos>().0.enabled = debug.0;
}
