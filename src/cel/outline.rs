use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, Face, RenderPipelineDescriptor, SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;

use super::{KeyLight, PoseRate};

/// Ink line color: near-black with a hint of warmth, like BTAS character lines.
const OUTLINE_COLOR: Color = Color::srgb(0.02, 0.01, 0.015);
/// Base line weight in physical pixels (the shader varies it ~0.4x–2x).
const OUTLINE_WIDTH_PX: f32 = 2.2;

/// Inverted-hull outline. See `assets/shaders/outline.wgsl`.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct OutlineMaterial {
    #[uniform(0)]
    color: LinearRgba,
    #[uniform(0)]
    width_px: f32,
    /// Unit direction toward the key light; drives thick-in-shadow ink weight.
    #[uniform(0)]
    to_light: Vec3,
    /// Character pose rate (Hz), so the line boils in step with the poses.
    #[uniform(0)]
    pose_rate: f32,
}

impl OutlineMaterial {
    pub fn new(to_light: Vec3, pose_rate: f32) -> Self {
        Self {
            color: OUTLINE_COLOR.into(),
            width_px: OUTLINE_WIDTH_PX,
            to_light,
            pose_rate,
        }
    }
}

impl Material for OutlineMaterial {
    fn vertex_shader() -> ShaderRef {
        "shaders/outline.wgsl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "shaders/outline.wgsl".into()
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        // Keep only the hull's back faces: they show as a rim around the
        // silhouette and are hidden behind the real mesh everywhere else.
        descriptor.primitive.cull_mode = Some(Face::Front);
        Ok(())
    }
}

/// A character's outline material (one per character, since each boils at its
/// own pose rate). On the `CelCharacter` root.
#[derive(Component)]
pub struct CelOutline(pub Handle<OutlineMaterial>);

/// Unit direction toward the key light; a directional light shines along its
/// forward (-Z), so toward it is back.
pub fn to_light(light: &GlobalTransform) -> Vec3 {
    light.back().as_vec3()
}

/// Mirrors each character's pose rate into its outline material.
pub fn sync_pose_rate(
    characters: Query<(&PoseRate, &CelOutline), Changed<PoseRate>>,
    mut materials: ResMut<Assets<OutlineMaterial>>,
) {
    for (rate, outline) in &characters {
        if let Some(mut mat) = materials.get_mut(&outline.0) {
            mat.pose_rate = rate.0;
        }
    }
}

/// Mirrors the key light's direction into every outline material.
pub fn sync_key_light(
    light: Single<&GlobalTransform, (With<KeyLight>, Changed<GlobalTransform>)>,
    outlines: Query<&CelOutline>,
    mut materials: ResMut<Assets<OutlineMaterial>>,
) {
    let dir = to_light(&light);
    for outline in &outlines {
        if let Some(mut mat) = materials.get_mut(&outline.0) {
            mat.to_light = dir;
        }
    }
}
