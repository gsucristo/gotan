use std::f32::consts::FRAC_PI_3;
use std::time::Duration;

use bevy::prelude::*;

use crate::physics::Velocity;
use crate::player::{Player, WALK_SPEED};

/// glTF (relative to `assets/`). Quaternius Universal Base Character (CC0) with
/// Universal Animation Library clips and a fedora on the `Head` bone, baked into
/// one GLB. See `models/char/tango_man_LICENSE.txt`.
pub const MODEL_PATH: &str = "models/char/tango_man.glb";

/// Model is ~1.87 units (m) tall with hat; scaled so the character stands ~100
/// world units — much taller than the 32-unit collider, which is fine: feet
/// still plant on the collider floor via FEET_OFFSET_Y.
pub const MODEL_SCALE: f32 = 54.0;
/// Horizontal squash on the model's own X/Z: turns the heroic build lanky.
const MODEL_SLIM: f32 = 0.85;
/// The collider is centered on the player transform, so drop the model by half
/// its height to plant the feet on the collider's bottom edge.
const FEET_OFFSET_Y: f32 = -16.0;

// Model forward is +Z (toward the camera). A ±60° turn about Y: near-profile
// facing screen right / left, opened 30° toward the camera so the face and
// chest (shirt V, hat brim) still read without looking too 3D.
fn face_right() -> Quat {
    Quat::from_rotation_y(FRAC_PI_3)
}
fn face_left() -> Quat {
    Quat::from_rotation_y(-FRAC_PI_3)
}

// Clip indices within tango_man.glb: Idle_Loop, Jump_Loop, Walk_Loop,
// Sprint_Loop.
const ANIM_IDLE: usize = 9;
const ANIM_JUMP: usize = 15;
const ANIM_WALK: usize = 42;
const ANIM_RUN: usize = 36;

/// Natural ground speed of the in-place locomotion clips, in model units (m/s):
/// how fast a planted foot slides back, i.e. how fast the body must move for
/// feet to look planted. Measured in Blender from foot-bone stance velocity.
const WALK_CLIP_SPEED: f32 = 1.02;
const RUN_CLIP_SPEED: f32 = 9.25;

/// Model units → world units along the model's forward (local Z) axis, which
/// is both scaled and slimmed.
const STRIDE_SCALE: f32 = MODEL_SCALE * MODEL_SLIM;

/// Cap on speed-matched playback, so a glitchy velocity spike can't spin the
/// clip absurdly fast.
const MAX_PLAYBACK: f32 = 3.0;

/// Speed (world units/s) above which the character is considered walking, used
/// for both the walk animation and the facing flip. Filters out collision jitter.
const MOVE_THRESHOLD: f32 = 15.0;
/// Speed above which the character runs instead of walks: a bit over walk speed,
/// so accel/decel between the two doesn't flicker the clip.
const RUN_THRESHOLD: f32 = WALK_SPEED * 1.15;

/// Stepped animation: poses are held instead of interpolated every render frame,
/// like hand-drawn cels at 24 fps. Smooth mocap motion is a big part of what
/// reads as "3D game". "On twos" = new pose every 2 film frames (12/s), "on
/// threes" = every 3 (8/s). Locomotion (walk, run) goes on twos — its clips are
/// sped up to match ground speed, so threes would leave too few poses per
/// stride — the rest on threes.
const ON_TWOS: f32 = 12.0;
const ON_THREES: f32 = 8.0;

/// The character's current pose rate (Hz). The outline shader boils its line
/// at this rate on the same clock, so line and pose change on the same frame.
#[derive(Resource)]
pub struct PoseRate(pub f32);

/// Last pose step applied, per rig: the rate it was counted at and its index
/// (`None` until the first frame).
#[derive(Component, Default)]
struct StepClock(Option<(f32, u32)>);

/// Marks the visual model child of the player. Carries the scale/offset and gets
/// rotated to face the movement direction.
#[derive(Component)]
pub struct CharModel;

/// Which clip the character is currently in, tracked so we only trigger a new
/// transition on an actual state change instead of restarting every frame.
#[derive(Component, Debug, PartialEq, Eq, Clone, Copy)]
enum CharState {
    Idle,
    Walk,
    Run,
    Jump,
}

impl CharState {
    /// Clip playback rate for a given ground speed: locomotion clips play at
    /// |speed| / clip natural speed so feet stay planted. Idle is frozen on the
    /// clip's first frame — a static standing pose, no idle motion — and jump
    /// plays at 1x.
    fn playback(self, speed: f32) -> f32 {
        let natural = match self {
            CharState::Walk => WALK_CLIP_SPEED,
            CharState::Run => RUN_CLIP_SPEED,
            CharState::Idle => return 0.0,
            CharState::Jump => return 1.0,
        } * STRIDE_SCALE;
        (speed.abs() / natural).min(MAX_PLAYBACK)
    }

    fn pose_rate(self) -> f32 {
        match self {
            CharState::Walk | CharState::Run => ON_TWOS,
            CharState::Idle | CharState::Jump => ON_THREES,
        }
    }
}

/// Graph handle + clip node indices, built once at startup.
#[derive(Resource)]
struct CharAnimations {
    graph: Handle<AnimationGraph>,
    idle: AnimationNodeIndex,
    walk: AnimationNodeIndex,
    run: AnimationNodeIndex,
    jump: AnimationNodeIndex,
}

pub struct CharacterPlugin;

impl Plugin for CharacterPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(PoseRate(ON_THREES))
            .add_systems(Startup, setup_animations).add_systems(
            Update,
            (
                attach_animation_graph,
                face_movement,
                (drive_animation, step_animation).chain(),
            ),
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
                .with_scale(Vec3::new(MODEL_SLIM, 1.0, MODEL_SLIM) * MODEL_SCALE)
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
        clip(ANIM_WALK),
        clip(ANIM_RUN),
        clip(ANIM_JUMP),
    ]);
    commands.insert_resource(CharAnimations {
        graph: graphs.add(graph),
        idle: nodes[0],
        walk: nodes[1],
        run: nodes[2],
        jump: nodes[3],
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
            StepClock::default(),
        ));
    }
}

/// Picks Idle / Walk / Run / Jump from the player's physics state and cross-fades to it
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
    } else if vel.x.abs() > RUN_THRESHOLD {
        CharState::Run
    } else if vel.x.abs() > MOVE_THRESHOLD {
        CharState::Walk
    } else {
        CharState::Idle
    };

    let (mut anim_player, mut transitions, mut state) = rig.into_inner();
    if *state == desired {
        return;
    }
    let (node, looping) = match desired {
        CharState::Idle => (anims.idle, true),
        CharState::Walk => (anims.walk, true),
        CharState::Run => (anims.run, true),
        CharState::Jump => (anims.jump, false),
    };
    // Hard cut, no cross-fade: cel animation cuts between actions, and a blend
    // would smear the held poses.
    let active = transitions.play(&mut anim_player, node, Duration::ZERO);
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

/// Steps the rig's clips at its state's pose rate, on the same clock the shaders
/// use for boil (`globals.time` = `Time::elapsed_secs_wrapped`): a pose step
/// happens whenever `floor(time * rate)` ticks over. The animation system
/// advances each clip by `delta * speed`, so hold frames get speed 0 and a step
/// frame gets the speed that jumps exactly the elapsed whole steps (times the
/// speed-matched playback rate, so walk/run feet stay planted). Looping stays
/// native. Also publishes the rate for the outline boil.
fn step_animation(
    time: Res<Time>,
    vel: Single<&Velocity, With<Player>>,
    mut pose_rate: ResMut<PoseRate>,
    mut rigs: Query<(&mut AnimationPlayer, &mut StepClock, &CharState)>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    for (mut player, mut clock, state) in &mut rigs {
        let rate = state.pose_rate();
        let now = (time.elapsed_secs_wrapped() * rate) as u32;
        let steps = match clock.0 {
            // Rate just changed: indices aren't comparable, restart the count.
            Some((last_rate, _)) if last_rate != rate => 0,
            // Hourly wrap resets the clock: count it as a single step.
            Some((_, last)) if now < last => 1,
            Some((_, last)) => now - last,
            None => 0,
        };
        clock.0 = Some((rate, now));
        let speed = steps as f32 / rate / dt * state.playback(vel.x);
        for (_, active) in player.playing_animations_mut() {
            active.set_speed(speed);
        }
        if pose_rate.0 != rate {
            pose_rate.0 = rate;
        }
    }
}
