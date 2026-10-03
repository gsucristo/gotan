//! The cel look: two-tone BTAS shading, ink outlines that boil in step with
//! stepped ("on twos/threes") animation, paper grain, black-paper lighting.
//! Self-contained — game code opts in with `CelCharacter` (glTF characters),
//! `KeyLight`, `camera_bundle` and the `flat` / `emissive` materials.

mod material;
mod outline;
mod post;
mod rig;

use bevy::light::DirectionalLightShadowMap;
use bevy::pbr::MaterialPlugin;
use bevy::prelude::*;

pub use material::{FlatMaterial, emissive, flat};
pub use post::camera_bundle;

/// Pose rates (Hz) for stepped animation: poses are held like hand-drawn cels at
/// 24 fps instead of interpolated every render frame — smooth mocap motion is a
/// big part of what reads as "3D game". "On twos" = new pose every 2 film frames
/// (12/s), "on threes" = every 3 (8/s).
pub const ON_TWOS: f32 = 12.0;
pub const ON_THREES: f32 = 8.0;

/// Opt-in for a glTF character model root (the entity holding `WorldAssetRoot`).
/// When its scene is ready, every mesh in it gets simplified normals, the flat
/// material (parts retinted by `palette`, keyed by glTF material name) and an
/// ink outline hull.
#[derive(Component)]
#[require(PoseRate)]
pub struct CelCharacter {
    pub palette: &'static [(&'static str, Color)],
}

/// The character's current pose rate (Hz). Its outline boils at this rate on
/// the shader clock (`globals.time` = `Time::elapsed_secs_wrapped`), so line and
/// pose change on the same frame. Whatever steps the animation writes it.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct PoseRate(pub f32);

impl Default for PoseRate {
    fn default() -> Self {
        Self(ON_THREES)
    }
}

/// The key directional light. `flat.wgsl` shades against
/// `directional_lights[0]`, so this must be the scene's only directional light;
/// outlines read its direction from here.
#[derive(Component)]
pub struct KeyLight;

/// Keeps the `boil` shader import (shared by flat.wgsl and outline.wgsl) loaded
/// so `#import boil::...` resolves.
#[derive(Resource)]
struct BoilShader(#[allow(dead_code)] Handle<Shader>);

pub struct CelPlugin;

impl Plugin for CelPlugin {
    fn build(&self, app: &mut App) {
        let boil = app.world().resource::<AssetServer>().load("shaders/boil.wgsl");
        app.insert_resource(BoilShader(boil))
            // Black paper: everything is painted light-onto-dark.
            .insert_resource(ClearColor(Color::BLACK))
            // High-res shadow map: more texels for the same cascade → crisper
            // hard edge.
            .insert_resource(DirectionalLightShadowMap { size: 8192 })
            .init_resource::<rig::SimplifiedMeshes>()
            .add_plugins((
                MaterialPlugin::<FlatMaterial>::default(),
                MaterialPlugin::<outline::OutlineMaterial>::default(),
            ))
            .add_observer(rig::on_character_ready)
            .add_systems(Update, (outline::sync_pose_rate, outline::sync_key_light));
    }
}
