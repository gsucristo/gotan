use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;

use crate::PHYSICS_TICK_HZ;

#[derive(Resource, Default)]
struct DebugConfig {
    visible: bool,
}

#[derive(Component)]
struct DebugText;

pub struct DebugPlugin;

impl Plugin for DebugPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DebugConfig>()
            .add_systems(Startup, spawn_debug_text)
            .add_systems(Update, (toggle_debug, update_debug_text));
    }
}

fn spawn_debug_text(mut commands: Commands) {
    // Screen-space UI node: stays pinned to the corner instead of drifting with
    // the camera the way a world-space Text2d would.
    commands.spawn((
        Text::new(String::new()),
        TextFont::from_font_size(14.0),
        TextColor(Color::srgb(0.6, 1.0, 0.6)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(8.0),
            left: Val::Px(8.0),
            ..default()
        },
        DebugText,
        Visibility::Hidden,
    ));
}

fn toggle_debug(
    input: Res<ButtonInput<KeyCode>>,
    mut config: ResMut<DebugConfig>,
    mut text: Single<&mut Visibility, With<DebugText>>,
) {
    if input.just_pressed(KeyCode::Backquote) {
        config.visible = !config.visible;
        **text = if config.visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

fn update_debug_text(
    diagnostics: Res<DiagnosticsStore>,
    fixed: Res<Time<Fixed>>,
    mut text: Single<&mut Text, With<DebugText>>,
) {
    let Some(fps) = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|d| d.smoothed())
    else {
        return;
    };
    let tick_ms = fixed.timestep().as_secs_f64() * 1000.0;
    text.0 = format!(
        "fps: {fps:.0}\nphysics: {tick_ms:.1}ms/tick @ {:.0}Hz",
        PHYSICS_TICK_HZ
    );
}