use std::f32::consts::FRAC_PI_2;
use std::time::Duration;

use bevy::prelude::*;

use crate::physics::Velocity;
use crate::player::Player;

/// glTF (relative to `assets/`). Self-contained Quaternius cowboy: one embedded
/// mesh + rig + animation clips, no external buffers/textures.
pub const MODEL_PATH: &str = "models/char/Cowboy_Male.gltf";

/// Model is ~3.5 units tall in its own space; scaled up so the cowboy reads at a
/// good size on screen (taller than the 32-unit collider, which is fine — feet
/// still plant on the collider floor via FEET_OFFSET_Y).
pub const MODEL_SCALE: f32 = 36.0;
/// The collider is centered on the player transform, so drop the model by half
/// its height to plant the feet on the collider's bottom edge.
const FEET_OFFSET_Y: f32 = -16.0;

// Model forward is +Z; rotating +90°/-90° about Y turns it to face +X / -X
// (screen right / left). Flip these two if the cowboy ends up back-to-front.
fn face_right() -> Quat {
    Quat::from_rotation_y(FRAC_PI_2)
}
fn face_left() -> Quat {
    Quat::from_rotation_y(-FRAC_PI_2)
}

// Clip indices within Cowboy_Male.gltf (verified from the file's animation list).
const ANIM_IDLE: usize = 2;
const ANIM_JUMP: usize = 3;
const ANIM_RUN: usize = 8;

/// Speed (world units/s) above which the character is considered walking, used
/// for both the run animation and the facing flip. Filters out collision jitter.
const MOVE_THRESHOLD: f32 = 15.0;

/// Marks the visual model child of the player. Carries the scale/offset and gets
/// rotated to face the movement direction.
#[derive(Component)]
pub struct CharModel;

/// Which clip the character is currently in, tracked so we only trigger a new
/// transition on an actual state change instead of restarting every frame.
#[derive(Component, Debug, PartialEq, Eq, Clone, Copy)]
enum CharState {
    Idle,
    Run,
    Jump,
}

/// Graph handle + clip node indices, built once at startup.
#[derive(Resource)]
struct CharAnimations {
    graph: Handle<AnimationGraph>,
    idle: AnimationNodeIndex,
    run: AnimationNodeIndex,
    jump: AnimationNodeIndex,
}

pub struct CharacterPlugin;

impl Plugin for CharacterPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_animations).add_systems(
            Update,
            (attach_animation_graph, face_movement, drive_animation),
        );
    }
}

/// Spawns the animated cowboy as a child of `player`, so the model rides the
/// physics transform while keeping its own scale, foot offset and facing.
pub fn spawn_model(commands: &mut Commands, player: Entity, asset_server: &AssetServer) {
    let scene = asset_server.load(GltfAssetLabel::Scene(0).from_asset(MODEL_PATH));
    commands.entity(player).with_children(|parent| {
        parent.spawn((
            CharModel,
            WorldAssetRoot(scene),
            Transform::from_xyz(0.0, FEET_OFFSET_Y, 0.0)
                .with_scale(Vec3::splat(MODEL_SCALE))
                .with_rotation(face_right()),
        ));
    });
}

/// Loads the three clips we drive from and bakes them into one animation graph.
fn setup_animations(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
) {
    let clip = |i: usize| asset_server.load(GltfAssetLabel::Animation(i).from_asset(MODEL_PATH));
    let (graph, nodes) = AnimationGraph::from_clips([
        clip(ANIM_IDLE),
        clip(ANIM_RUN),
        clip(ANIM_JUMP),
    ]);
    commands.insert_resource(CharAnimations {
        graph: graphs.add(graph),
        idle: nodes[0],
        run: nodes[1],
        jump: nodes[2],
    });
}

/// The glTF loader spawns an `AnimationPlayer` on the rig's armature (a deep
/// descendant of the model). When one appears, wire it to our graph, give it an
/// `AnimationTransitions` for cross-fading, and start it idling.
fn attach_animation_graph(
    mut commands: Commands,
    anims: Res<CharAnimations>,
    mut players: Query<(Entity, &mut AnimationPlayer), Added<AnimationPlayer>>,
) {
    for (entity, mut player) in &mut players {
        let mut transitions = AnimationTransitions::new();
        transitions
            .play(&mut player, anims.idle, Duration::ZERO)
            .repeat();
        commands.entity(entity).insert((
            AnimationGraphHandle(anims.graph.clone()),
            transitions,
            CharState::Idle,
        ));
    }
}

/// Picks Idle / Run / Jump from the player's physics state and cross-fades to it
/// on change. Single character, so the two `Single`s (physics body vs. armature)
/// resolve unambiguously and the system simply skips until the rig exists.
fn drive_animation(
    body: Single<(&Player, &Velocity)>,
    anims: Res<CharAnimations>,
    rig: Single<(&mut AnimationPlayer, &mut AnimationTransitions, &mut CharState)>,
) {
    let (player, vel) = *body;
    let desired = if !player.on_ground {
        CharState::Jump
    } else if vel.x.abs() > MOVE_THRESHOLD {
        CharState::Run
    } else {
        CharState::Idle
    };

    let (mut anim_player, mut transitions, mut state) = rig.into_inner();
    if *state == desired {
        return;
    }
    let (node, looping) = match desired {
        CharState::Idle => (anims.idle, true),
        CharState::Run => (anims.run, true),
        CharState::Jump => (anims.jump, false),
    };
    let active = transitions.play(&mut anim_player, node, Duration::from_millis(150));
    if looping {
        active.repeat();
    }
    *state = desired;
}

/// Turns the model to face the way it's moving; holds the last facing when idle.
fn face_movement(
    vel: Single<&Velocity, With<Player>>,
    mut model: Single<&mut Transform, With<CharModel>>,
) {
    if vel.x > MOVE_THRESHOLD {
        model.rotation = face_right();
    } else if vel.x < -MOVE_THRESHOLD {
        model.rotation = face_left();
    }
}
