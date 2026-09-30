use bevy::gltf::GltfMaterialName;
use bevy::light::NotShadowCaster;
use bevy::mesh::skinning::SkinnedMesh;
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, Face, RenderPipelineDescriptor, SpecializedMeshPipelineError,
};
use bevy::camera::visibility::NoFrustumCulling;
use bevy::shader::ShaderRef;

use crate::character::PoseRate;
use crate::flat;

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

/// Marks a mesh that already got its outline hull.
#[derive(Component)]
struct Outlined;

#[derive(Resource)]
struct OutlineHandle(Handle<OutlineMaterial>);

pub struct OutlinePlugin;

impl Plugin for OutlinePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<OutlineMaterial>::default())
            .add_systems(Startup, setup)
            .add_systems(Update, (add_outlines, sync_light, sync_pose_rate));
    }
}

fn setup(mut commands: Commands, mut materials: ResMut<Assets<OutlineMaterial>>) {
    commands.insert_resource(OutlineHandle(materials.add(OutlineMaterial {
        color: OUTLINE_COLOR.into(),
        width_px: OUTLINE_WIDTH_PX,
        to_light: Vec3::Y,
        pose_rate: 8.0,
    })));
}

/// Mirrors the character's pose rate into the outline material.
fn sync_pose_rate(
    outline: Res<OutlineHandle>,
    rate: Res<PoseRate>,
    mut materials: ResMut<Assets<OutlineMaterial>>,
) {
    if !rate.is_changed() {
        return;
    }
    if let Some(mut mat) = materials.get_mut(&outline.0) {
        mat.pose_rate = rate.0;
    }
}

/// Mirrors the key light's direction into the outline material.
fn sync_light(
    outline: Res<OutlineHandle>,
    light: Single<&GlobalTransform, (With<DirectionalLight>, Changed<GlobalTransform>)>,
    mut materials: ResMut<Assets<OutlineMaterial>>,
) {
    if let Some(mut mat) = materials.get_mut(&outline.0) {
        // A directional light shines along its forward (-Z); toward it is back.
        mat.to_light = light.back().as_vec3();
    }
}

/// Prepares every glTF mesh (only the character is glTF) for the cel look:
/// swaps in a copy with simplified normals (`flat::simplify_normals`, bolder
/// shadow shapes), then gives it an outline hull: a child
/// sharing the same mesh — and skin, so it animates in lockstep — drawn with
/// `OutlineMaterial`. Child with identity transform, so unskinned parts (the
/// hat on the head bone) follow their parent too.
fn add_outlines(
    mut commands: Commands,
    outline: Res<OutlineHandle>,
    mut mesh_assets: ResMut<Assets<Mesh>>,
    meshes: Query<(Entity, &Mesh3d, Option<&SkinnedMesh>), (With<GltfMaterialName>, Without<Outlined>)>,
) {
    for (entity, mesh, skin) in &meshes {
        // Mesh asset may not be loaded yet; retry next frame.
        let Some(source) = mesh_assets.get(&mesh.0) else {
            continue;
        };
        let mesh = match flat::simplify_normals(source) {
            Some(simplified) => Mesh3d(mesh_assets.add(simplified)),
            None => mesh.clone(),
        };
        commands.entity(entity).insert((Outlined, mesh.clone()));
        let mut hull = commands.spawn((
            ChildOf(entity),
            mesh.clone(),
            MeshMaterial3d(outline.0.clone()),
            Transform::default(),
            NotShadowCaster,
            // The extruded, animated hull can leave the rest-pose bounds.
            NoFrustumCulling,
        ));
        if let Some(skin) = skin {
            hull.insert(skin.clone());
        }
    }
}
