//! The player: a character driven by `MoveIntent`, with coyote time, jump
//! buffering and variable jump height.

use bevy::prelude::*;

use crate::camera::CameraTarget;
use crate::character::{self, Character};
use crate::input::MoveIntent;
use crate::physics::{Grounded, MotionSystems, Velocity};

const GROUND_ACCEL: f32 = 3400.0;
const AIR_ACCEL: f32 = 2000.0;
const JUMP_VELOCITY: f32 = 820.0;
/// Share of vertical speed kept when jump is released mid-rise.
const JUMP_CUT: f32 = 0.45;
const COYOTE_TIME: f32 = 0.1;
const JUMP_BUFFER_TIME: f32 = 0.12;

#[derive(Component, Debug)]
pub struct Player {
    coyote: Timer,
    jump_buffer: Timer,
}

impl Default for Player {
    fn default() -> Self {
        Self {
            // Start spent: a fresh `Once` timer reports `!is_finished()`, which
            // the jump check reads as an open window — so unspent timers would
            // fire a jump on the very first tick with no input.
            coyote: spent_timer(COYOTE_TIME),
            jump_buffer: spent_timer(JUMP_BUFFER_TIME),
        }
    }
}

fn spent_timer(secs: f32) -> Timer {
    let mut t = Timer::from_seconds(secs, TimerMode::Once);
    consume(&mut t);
    t
}

fn consume(t: &mut Timer) {
    let d = t.duration();
    t.set_elapsed(d);
}

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedUpdate, apply_movement.in_set(MotionSystems::Intent));
    }
}

/// Spawns the player character with its feet at `feet`.
pub fn spawn(commands: &mut Commands, assets: &AssetServer, feet: Vec2) -> Entity {
    let def = &character::TANGO_MAN;
    let body = character::spawn(commands, assets, def, feet);
    commands.entity(body).insert((
        Player::default(),
        CameraTarget {
            anchor: Vec2::NEG_Y * def.body.y / 2.0,
        },
    ));
    body
}

fn apply_movement(
    time: Res<Time>,
    intent: Res<MoveIntent>,
    player: Single<(&Character, &Grounded, &mut Velocity, &mut Player)>,
) {
    let dt = time.delta_secs();
    let (Character(def), grounded, mut vel, mut p) = player.into_inner();

    p.coyote.tick(time.delta());
    p.jump_buffer.tick(time.delta());

    if intent.jump_pressed {
        p.jump_buffer.reset();
    }
    if grounded.0 {
        p.coyote.reset();
    }

    let accel = if grounded.0 { GROUND_ACCEL } else { AIR_ACCEL };
    let speed = if intent.sprint { def.run_speed } else { def.walk_speed };
    let target_x = intent.move_x * speed;
    let step = accel * dt;
    vel.x += (target_x - vel.x).clamp(-step, step);

    // Variable jump height: letting go of jump mid-rise cuts vertical speed.
    if intent.jump_released && vel.y > 0.0 {
        vel.y *= JUMP_CUT;
    }

    // Jump: needs both an open input buffer and an open coyote window.
    if !p.jump_buffer.is_finished() && !p.coyote.is_finished() {
        vel.y = JUMP_VELOCITY;
        consume(&mut p.coyote);
        consume(&mut p.jump_buffer);
    }
}
