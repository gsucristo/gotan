use bevy::input::gamepad::{Gamepad, GamepadAxis, GamepadButton};
use bevy::prelude::*;

const STICK_DEADZONE: f32 = 0.1;

#[derive(Resource, Debug, Default, Clone)]
pub struct MoveIntent {
    pub move_x: f32,
    pub jump_pressed: bool,
    pub jump_released: bool,
}

pub fn read_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    mut intent: ResMut<MoveIntent>,
) {
    intent.move_x = 0.0;
    intent.jump_pressed = false;
    intent.jump_released = false;

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
    }

    intent.move_x = (kb + g.clamp(-1.0, 1.0)).clamp(-1.0, 1.0);
    intent.jump_pressed = gp_jump || keyboard.just_pressed(KeyCode::Space);
    intent.jump_released = gp_jump_release || keyboard.just_released(KeyCode::Space);
}