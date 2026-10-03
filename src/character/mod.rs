//! Animated glTF characters: a kinematic physics body with the model riding it
//! as a child. Whatever drives the body (player input, AI) just writes its
//! `Velocity`; facing and animation follow from that.

mod anim;

use std::f32::consts::FRAC_PI_3;

use bevy::gltf::Gltf;
use bevy::prelude::*;

use crate::cel::CelCharacter;
use crate::physics::{KinematicBody, Velocity};

/// Static description of a character type.
pub struct CharacterDef {
    /// glTF path, relative to `assets/`.
    pub model: &'static str,
    /// Model units → world units.
    pub scale: f32,
    /// Horizontal squash on the model's own X/Z.
    pub slim: f32,
    /// Physics box (world units). The model's feet are planted on its bottom edge.
    pub body: Vec2,
    /// Ground speeds (world units/s); locomotion clips are playback-scaled to match.
    pub walk_speed: f32,
    pub run_speed: f32,
    pub clips: Clips,
    /// Per-part color overrides, keyed by glTF material name.
    pub palette: &'static [(&'static str, Color)],
}

/// Named glTF animation clips.
pub struct Clips {
    pub idle: &'static str,
    pub walk: &'static str,
    pub run: &'static str,
    pub jump: &'static str,
    /// Natural ground speed of the in-place walk / run clips, in model units
    /// (m/s): how fast a planted foot slides back, i.e. how fast the body must
    /// move for feet to look planted.
    pub walk_ground_speed: f32,
    pub run_ground_speed: f32,
}

/// Quaternius Universal Base Character (CC0) with Universal Animation Library
/// clips and a fedora on the `Head` bone, baked into one GLB. See
/// `models/char/tango_man_LICENSE.txt`.
pub const TANGO_MAN: CharacterDef = CharacterDef {
    model: "models/char/tango_man.glb",
    // ~1.87 m tall with hat → stands ~100 world units.
    scale: 54.0,
    // Turns the heroic build lanky.
    slim: 0.85,
    body: Vec2::new(28.0, 90.0),
    // Walk is ~0.8 body heights/s — a brisk walk — and run a full sprint.
    walk_speed: 80.0,
    run_speed: 340.0,
    clips: Clips {
        idle: "Idle_Loop",
        walk: "Walk_Loop",
        run: "Sprint_Loop",
        jump: "Jump_Loop",
        // Measured in Blender from foot-bone stance velocity.
        walk_ground_speed: 1.02,
        run_ground_speed: 9.25,
    },
    // Tango dancer: black suit + fedora, red shirt + hat band, tan skin.
    // "Black" cloth is painted as dark slate (BTAS convention) so the lit and
    // shadow tones stay distinct; true black is left to the shadow. Parts:
    // Skin, Jacket, Pants, Shirt, Shoes, Hat, HatBand, Hair, Eyes.
    palette: &[
        ("Skin", Color::srgb(0.80, 0.56, 0.34)),
        ("Jacket", Color::srgb(0.20, 0.21, 0.27)),
        ("Pants", Color::srgb(0.20, 0.21, 0.27)),
        ("Shoes", Color::srgb(0.10, 0.10, 0.12)),
        ("Shirt", Color::srgb(0.62, 0.05, 0.05)),
        ("Hat", Color::srgb(0.16, 0.16, 0.21)),
        ("HatBand", Color::srgb(0.62, 0.05, 0.05)),
        ("Hair", Color::srgb(0.03, 0.03, 0.04)),
        ("Eyes", Color::srgb(0.02, 0.02, 0.02)),
    ],
};

/// Speed (world units/s) above which a character counts as moving, for both the
/// walk animation and the facing flip. Filters out collision jitter.
const MOVE_THRESHOLD: f32 = 15.0;

/// A character's physics body (the root entity).
#[derive(Component)]
// Visibility on the body lets the model child inherit it.
#[require(Visibility)]
pub struct Character(pub &'static CharacterDef);

/// The model child of a character body.
#[derive(Component)]
struct CharacterModel {
    body: Entity,
    gltf: Handle<Gltf>,
}

pub struct CharacterPlugin;

impl Plugin for CharacterPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(anim::on_model_ready).add_systems(
            Update,
            (face_movement, (anim::drive_animation, anim::step_animation).chain()),
        );
    }
}

/// Spawns a character standing with its feet at `feet`; returns the body.
pub fn spawn(
    commands: &mut Commands,
    assets: &AssetServer,
    def: &'static CharacterDef,
    feet: Vec2,
) -> Entity {
    let center = feet + Vec2::Y * def.body.y / 2.0;
    let body = commands
        .spawn((
            Character(def),
            KinematicBody { size: def.body },
            Transform::from_translation(center.extend(0.0)),
        ))
        .id();
    commands.spawn((
        ChildOf(body),
        CharacterModel {
            body,
            gltf: assets.load(def.model),
        },
        WorldAssetRoot(assets.load(GltfAssetLabel::Scene(0).from_asset(def.model))),
        CelCharacter {
            palette: def.palette,
        },
        Transform::from_xyz(0.0, -def.body.y / 2.0, 0.0)
            .with_scale(Vec3::new(def.slim, 1.0, def.slim) * def.scale)
            .with_rotation(facing(1.0)),
    ));
    body
}

/// Model forward is +Z (toward the camera). A ±60° turn about Y: near-profile
/// facing screen right / left, opened 30° toward the camera so the face and
/// chest (shirt V, hat brim) still read without looking too 3D.
fn facing(dir: f32) -> Quat {
    Quat::from_rotation_y(FRAC_PI_3 * dir.signum())
}

/// Turns each model to face the way its body moves; holds the last facing when
/// idle.
fn face_movement(bodies: Query<&Velocity>, mut models: Query<(&CharacterModel, &mut Transform)>) {
    for (model, mut tf) in &mut models {
        let Ok(vel) = bodies.get(model.body) else {
            continue;
        };
        if vel.x.abs() > MOVE_THRESHOLD {
            tf.rotation = facing(vel.x);
        }
    }
}
