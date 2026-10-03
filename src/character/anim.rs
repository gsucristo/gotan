//! Clip selection and stepped (cel-timed) playback, per character rig.

use std::time::Duration;

use bevy::gltf::Gltf;
use bevy::prelude::*;
use bevy::world_serialization::WorldInstanceReady;

use super::{Character, CharacterDef, CharacterModel, MOVE_THRESHOLD};
use crate::cel::{ON_THREES, ON_TWOS, PoseRate};
use crate::physics::{Grounded, Velocity};

/// Run instead of walk above this multiple of walk speed: a bit over, so
/// accel/decel between the two doesn't flicker the clip.
const RUN_ABOVE_WALK: f32 = 1.15;

/// Cap on speed-matched playback, so a glitchy velocity spike can't spin the
/// clip absurdly fast.
const MAX_PLAYBACK: f32 = 3.0;

/// On the rig entity (the one holding the glTF's `AnimationPlayer`): the body it
/// animates, the model root (which carries its `PoseRate`), and its clip nodes.
#[derive(Component)]
pub(super) struct Rig {
    body: Entity,
    model: Entity,
    idle: AnimationNodeIndex,
    walk: AnimationNodeIndex,
    run: AnimationNodeIndex,
    jump: AnimationNodeIndex,
}

/// Which clip the rig is in, tracked so a transition only fires on an actual
/// state change instead of restarting every frame.
#[derive(Component, Debug, PartialEq, Eq, Clone, Copy)]
pub(super) enum CharState {
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
    fn playback(self, def: &CharacterDef, speed: f32) -> f32 {
        let natural = match self {
            CharState::Walk => def.clips.walk_ground_speed,
            CharState::Run => def.clips.run_ground_speed,
            CharState::Idle => return 0.0,
            CharState::Jump => return 1.0,
        };
        // Model units → world units along the model's forward (local Z) axis,
        // which is both scaled and slimmed.
        let stride_scale = def.scale * def.slim;
        (speed.abs() / (natural * stride_scale)).min(MAX_PLAYBACK)
    }

    /// Locomotion goes on twos — its clips are sped up to match ground speed,
    /// so threes would leave too few poses per stride — the rest on threes.
    fn pose_rate(self) -> f32 {
        match self {
            CharState::Walk | CharState::Run => ON_TWOS,
            CharState::Idle | CharState::Jump => ON_THREES,
        }
    }
}

/// Last pose step applied: the rate it was counted at and its index (`None`
/// until the first frame).
#[derive(Component, Default)]
pub(super) struct StepClock(Option<(f32, u32)>);

/// When a character model's scene is ready, builds its animation graph from the
/// def's named clips and wires it to the rig's `AnimationPlayer` (which the glTF
/// loader puts on the armature, a deep descendant of the model), idling.
#[allow(clippy::too_many_arguments)]
pub(super) fn on_model_ready(
    ready: On<WorldInstanceReady>,
    models: Query<&CharacterModel>,
    bodies: Query<&Character>,
    children: Query<&Children>,
    mut players: Query<&mut AnimationPlayer>,
    gltfs: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut commands: Commands,
) {
    let model_entity = ready.entity;
    let Ok(model) = models.get(model_entity) else {
        return;
    };
    let Ok(Character(def)) = bodies.get(model.body) else {
        return;
    };
    let Some(gltf) = gltfs.get(&model.gltf) else {
        warn!("character: {} not loaded at scene ready", def.model);
        return;
    };
    let clip = |name: &str| {
        let handle = gltf.named_animations.get(name).cloned();
        if handle.is_none() {
            warn!("character: {} has no clip {name:?}", def.model);
        }
        handle
    };
    let c = &def.clips;
    let (Some(idle), Some(walk), Some(run), Some(jump)) =
        (clip(c.idle), clip(c.walk), clip(c.run), clip(c.jump))
    else {
        return;
    };
    let (graph, nodes) = AnimationGraph::from_clips([idle, walk, run, jump]);
    let Some(rig) = children
        .iter_descendants(model_entity)
        .find(|e| players.contains(*e))
    else {
        warn!("character: {} has no AnimationPlayer", def.model);
        return;
    };
    let Ok(mut player) = players.get_mut(rig) else {
        return;
    };

    let mut transitions = AnimationTransitions::new();
    transitions
        .play(&mut player, nodes[0], Duration::ZERO)
        .repeat();
    commands.entity(rig).insert((
        AnimationGraphHandle(graphs.add(graph)),
        transitions,
        CharState::Idle,
        StepClock::default(),
        Rig {
            body: model.body,
            model: model_entity,
            idle: nodes[0],
            walk: nodes[1],
            run: nodes[2],
            jump: nodes[3],
        },
    ));
}

/// Picks Idle / Walk / Run / Jump from the body's physics state and cuts to it
/// on change.
pub(super) fn drive_animation(
    bodies: Query<(&Character, &Velocity, &Grounded)>,
    mut rigs: Query<(
        &Rig,
        &mut AnimationPlayer,
        &mut AnimationTransitions,
        &mut CharState,
    )>,
) {
    for (rig, mut player, mut transitions, mut state) in &mut rigs {
        let Ok((Character(def), vel, grounded)) = bodies.get(rig.body) else {
            continue;
        };
        let speed = vel.x.abs();
        let desired = if !grounded.0 {
            CharState::Jump
        } else if speed > def.walk_speed * RUN_ABOVE_WALK {
            CharState::Run
        } else if speed > MOVE_THRESHOLD {
            CharState::Walk
        } else {
            CharState::Idle
        };
        if *state == desired {
            continue;
        }
        let (node, looping) = match desired {
            CharState::Idle => (rig.idle, true),
            CharState::Walk => (rig.walk, true),
            CharState::Run => (rig.run, true),
            CharState::Jump => (rig.jump, false),
        };
        // Hard cut, no cross-fade: cel animation cuts between actions, and a
        // blend would smear the held poses.
        let active = transitions.play(&mut player, node, Duration::ZERO);
        if looping {
            active.repeat();
        }
        *state = desired;
    }
}

/// Steps each rig's clips at its state's pose rate, on the same clock the
/// shaders use for boil (`globals.time` = `Time::elapsed_secs_wrapped`): a pose
/// step happens whenever `floor(time * rate)` ticks over. The animation system
/// advances each clip by `delta * speed`, so hold frames get speed 0 and a step
/// frame gets the speed that jumps exactly the elapsed whole steps (times the
/// speed-matched playback rate, so walk/run feet stay planted). Looping stays
/// native. Also publishes the rate to the model's `PoseRate` for the outline
/// boil.
pub(super) fn step_animation(
    time: Res<Time>,
    bodies: Query<(&Character, &Velocity)>,
    mut rigs: Query<(&Rig, &mut AnimationPlayer, &mut StepClock, &CharState)>,
    mut pose_rates: Query<&mut PoseRate>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    for (rig, mut player, mut clock, state) in &mut rigs {
        let Ok((Character(def), vel)) = bodies.get(rig.body) else {
            continue;
        };
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
        let speed = steps as f32 / rate / dt * state.playback(def, vel.x);
        for (_, active) in player.playing_animations_mut() {
            active.set_speed(speed);
        }
        if let Ok(mut pose_rate) = pose_rates.get_mut(rig.model) {
            pose_rate.set_if_neq(PoseRate(rate));
        }
    }
}
