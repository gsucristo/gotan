//! Game state and pause. Pausing stops `Time<Virtual>`, which freezes
//! everything on game time at once: fixed ticks (physics, movement), animation,
//! camera follow, searchlights, and the shader clock (boil, film flicker).

use bevy::prelude::*;

#[derive(States, Debug, Default, Clone, PartialEq, Eq, Hash)]
pub enum GameState {
    #[default]
    Playing,
    Paused,
}

pub struct StatePlugin;

impl Plugin for StatePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GameState>()
            .add_systems(Update, toggle_pause)
            .add_systems(OnEnter(GameState::Paused), (pause_time, spawn_pause_label))
            .add_systems(OnExit(GameState::Paused), resume_time);
    }
}

fn toggle_pause(
    keys: Res<ButtonInput<KeyCode>>,
    state: Res<State<GameState>>,
    mut next: ResMut<NextState<GameState>>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        next.set(match state.get() {
            GameState::Playing => GameState::Paused,
            GameState::Paused => GameState::Playing,
        });
    }
}

fn pause_time(mut time: ResMut<Time<Virtual>>) {
    time.pause();
}

fn resume_time(mut time: ResMut<Time<Virtual>>) {
    time.unpause();
}

fn spawn_pause_label(mut commands: Commands) {
    commands.spawn((
        DespawnOnExit(GameState::Paused),
        Node {
            width: percent(100.0),
            height: percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        children![(
            Text::new("PAUSED"),
            TextFont::from_font_size(32.0),
            TextColor(Color::srgb(0.85, 0.8, 0.7)),
        )],
    ));
}
