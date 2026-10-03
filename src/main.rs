mod backdrop;
mod camera;
mod cel;
mod character;
mod debug;
mod input;
mod level;
mod physics;
mod player;
mod state;

use bevy::prelude::*;
use bevy::window::{PresentMode, Window, WindowPlugin};

fn main() -> AppExit {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "bevy-platformer".into(),
                present_mode: PresentMode::AutoNoVsync,
                ..default()
            }),
            ..default()
        }))
        .add_plugins((
            state::StatePlugin,
            input::ControlsPlugin,
            physics::PhysicsPlugin,
            cel::CelPlugin,
            character::CharacterPlugin,
            player::PlayerPlugin,
            camera::CameraPlugin,
            level::LevelPlugin,
            backdrop::BackdropPlugin,
            debug::DebugPlugin,
        ))
        .run()
}
