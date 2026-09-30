use bevy::prelude::*;

use crate::physics::Velocity;

/// Ground speeds (world units/s). The character is ~100 units tall, so walk is
/// ~0.8 body heights/s — a brisk walk — and sprint (Shift) is a full run. The
/// walk/run clips are playback-scaled to match (see character.rs).
pub const WALK_SPEED: f32 = 80.0;
pub const SPRINT_SPEED: f32 = 340.0;
const GROUND_ACCEL: f32 = 3400.0;
const AIR_ACCEL: f32 = 2000.0;
const JUMP_VELOCITY: f32 = 820.0;
const COYOTE_TIME: f32 = 0.1;
const JUMP_BUFFER_TIME: f32 = 0.12;

#[derive(Component, Debug)]
pub struct Player {
    pub on_ground: bool,
    pub coyote: Timer,
    pub jump_buffer: Timer,
}

impl Default for Player {
    fn default() -> Self {
        Self {
            on_ground: false,
            // Start spent: a fresh `Once` timer reports `!is_finished()`, which
            // our jump check reads as an open window — so unspent timers would
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

pub fn apply_movement(
    time: Res<Time<Fixed>>,
    intent: Res<crate::input::MoveIntent>,
    q: Single<(&mut Velocity, &mut Player)>,
) {
    let dt = time.delta_secs();
    let (mut vel, mut p) = q.into_inner();

    p.coyote.tick(time.delta());
    p.jump_buffer.tick(time.delta());

    if intent.jump_pressed {
        p.jump_buffer.reset();
    }
    if p.on_ground {
        p.coyote.reset();
    }

    let accel = if p.on_ground { GROUND_ACCEL } else { AIR_ACCEL };
    let speed = if intent.sprint { SPRINT_SPEED } else { WALK_SPEED };
    let target_x = intent.move_x * speed;
    let diff = target_x - vel.x;
    let step = accel * dt;
    vel.x += diff.clamp(-step, step);

    // Variable jump height: letting go of jump mid-rise cuts vertical speed.
    if intent.jump_released && vel.y > 0.0 {
        vel.y *= 0.45;
    }

    // Jump: consume the fresher of the input buffer plus the coyote window.
    if !p.jump_buffer.is_finished() && !p.coyote.is_finished() {
        vel.y = JUMP_VELOCITY;
        p.on_ground = false;
        consume(&mut p.coyote);
        consume(&mut p.jump_buffer);
    }
}