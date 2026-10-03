use bevy::input::gamepad::{Gamepad, GamepadAxis, GamepadButton};
use bevy::prelude::*;

use crate::physics::MotionSystems;
use crate::state::GameState;

const STICK_DEADZONE: f32 = 0.1;

/// Player intent, sampled once per rendered frame and read by the fixed tick.
/// `move_x` and `sprint` are levels, refreshed every frame. The jump fields are
/// edges, which `ButtonInput` only reports for one frame — but the fixed loop
/// runs 0..N times per frame, so they're latched here until a tick consumes
/// them (`consume_edges`): none lost on tickless frames, none repeated across
/// multiple ticks in one frame.
#[derive(Resource, Debug, Default, Clone)]
pub struct MoveIntent {
    pub move_x: f32,
    pub jump_pressed: bool,
    pub jump_released: bool,
    /// Sprint held (Shift / gamepad West).
    pub sprint: bool,
}

pub struct ControlsPlugin;

impl Plugin for ControlsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MoveIntent>()
            .add_systems(
                RunFixedMainLoop,
                read_input
                    .in_set(RunFixedMainLoopSystems::BeforeFixedMainLoop)
                    .run_if(in_state(GameState::Playing)),
            )
            .add_systems(FixedUpdate, consume_edges.after(MotionSystems::Intent))
            // Nothing latched while paused fires on resume.
            .add_systems(OnExit(GameState::Playing), clear);
    }
}

/// Every frame, just before the fixed loop.
fn read_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    mut intent: ResMut<MoveIntent>,
) {
    let mut kb = 0.0;
    if keyboard.pressed(KeyCode::KeyA) || keyboard.pressed(KeyCode::ArrowLeft) {
        kb -= 1.0;
    }
    if keyboard.pressed(KeyCode::KeyD) || keyboard.pressed(KeyCode::ArrowRight) {
        kb += 1.0;
    }

    let mut g = 0.0;
    let mut gp_jump = false;
    let mut gp_jump_release = false;
    let mut gp_sprint = false;
    for gp in &gamepads {
        let stick = gp.get(GamepadAxis::LeftStickX).unwrap_or(0.0);
        let dpad = gp
            .get(GamepadButton::DPadRight)
            .unwrap_or(0.0)
            - gp.get(GamepadButton::DPadLeft).unwrap_or(0.0);
        if stick.abs() > STICK_DEADZONE {
            g += stick;
        }
        g += dpad;
        if gp.just_pressed(GamepadButton::South) {
            gp_jump = true;
        }
        if gp.just_released(GamepadButton::South) {
            gp_jump_release = true;
        }
        if gp.pressed(GamepadButton::West) {
            gp_sprint = true;
        }
    }

    intent.move_x = (kb + g.clamp(-1.0, 1.0)).clamp(-1.0, 1.0);
    intent.jump_pressed |= gp_jump || keyboard.just_pressed(KeyCode::Space);
    intent.jump_released |= gp_jump_release || keyboard.just_released(KeyCode::Space);
    intent.sprint = gp_sprint
        || keyboard.pressed(KeyCode::ShiftLeft)
        || keyboard.pressed(KeyCode::ShiftRight);
}

/// Fixed tick, after every reader (`MotionSystems::Intent`): clears the latched
/// edges so each press/release acts on exactly one tick.
fn consume_edges(mut intent: ResMut<MoveIntent>) {
    intent.jump_pressed = false;
    intent.jump_released = false;
}

fn clear(mut intent: ResMut<MoveIntent>) {
    *intent = MoveIntent::default();
}
