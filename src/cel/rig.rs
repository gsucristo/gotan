//! Cel prep for glTF characters, run once per `CelCharacter` when its scene is
//! ready — scoped to that model's subtree, so other glTFs are left alone.

use bevy::camera::visibility::NoFrustumCulling;
use bevy::gltf::GltfMaterialName;
use bevy::light::NotShadowCaster;
use bevy::mesh::VertexAttributeValues;
use bevy::mesh::skinning::SkinnedMesh;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::world_serialization::WorldInstanceReady;

use super::outline::{self, CelOutline, OutlineMaterial};
use super::{CelCharacter, FlatMaterial, KeyLight, PoseRate, material};

/// How far character normals are bent toward a simple vertical-cylinder shape
/// (0 = original anatomy, 1 = pure cylinder). Shadow shapes then follow big
/// forms — one lit side, one dark side — instead of every muscle, like the
/// broad painted shadow planes in BTAS.
const NORMAL_SIMPLIFY: f32 = 0.55;

/// Simplified copies of source meshes, so characters sharing a model share them.
#[derive(Resource, Default)]
pub struct SimplifiedMeshes(HashMap<AssetId<Mesh>, Handle<Mesh>>);

/// For each mesh part of the model: swaps in simplified normals (bolder shadow
/// shapes), swaps its `StandardMaterial` for a `FlatMaterial` (cloning the base
/// so alpha/settings survive, applying any palette override), and gives it an
/// outline hull: a child sharing the same mesh — and skin, so it animates in
/// lockstep — drawn with the character's `OutlineMaterial`. Child with identity
/// transform, so unskinned parts (the hat on the head bone) follow their parent
/// too. Parts stay shadow casters and receivers, so the character self-shadows
/// and goes dark under a building's shadow.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn on_character_ready(
    ready: On<WorldInstanceReady>,
    roots: Query<(&CelCharacter, &PoseRate)>,
    children: Query<&Children>,
    parts: Query<(
        &Mesh3d,
        &MeshMaterial3d<StandardMaterial>,
        Option<&GltfMaterialName>,
        Option<&SkinnedMesh>,
    )>,
    key_light: Query<&GlobalTransform, With<KeyLight>>,
    std_materials: Res<Assets<StandardMaterial>>,
    mut flat_materials: ResMut<Assets<FlatMaterial>>,
    mut outline_materials: ResMut<Assets<OutlineMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut simplified: ResMut<SimplifiedMeshes>,
    mut commands: Commands,
) {
    let root = ready.entity;
    let Ok((cel, pose_rate)) = roots.get(root) else {
        return;
    };
    let to_light = key_light.single().map(outline::to_light).unwrap_or(Vec3::Y);
    let outline = outline_materials.add(OutlineMaterial::new(to_light, pose_rate.0));
    commands.entity(root).insert(CelOutline(outline.clone()));

    for part in children.iter_descendants(root) {
        let Ok((mesh, material, name, skin)) = parts.get(part) else {
            continue;
        };
        let Some(base) = std_materials.get(&material.0) else {
            warn!("cel: material of {part} not loaded at scene ready; left unconverted");
            continue;
        };
        let mut base = base.clone();
        let palette_color = name.and_then(|n| cel.palette.iter().find(|(p, _)| *p == n.0));
        if let Some((_, color)) = palette_color {
            base.base_color = *color;
        }
        let flat = flat_materials.add(material::from_standard(base));
        let mesh = Mesh3d(simplified_mesh(&mut meshes, &mut simplified, &mesh.0));

        commands
            .entity(part)
            .remove::<MeshMaterial3d<StandardMaterial>>()
            .insert((MeshMaterial3d(flat), mesh.clone()));
        let mut hull = commands.spawn((
            ChildOf(part),
            mesh,
            MeshMaterial3d(outline.clone()),
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

fn simplified_mesh(
    meshes: &mut Assets<Mesh>,
    cache: &mut SimplifiedMeshes,
    source: &Handle<Mesh>,
) -> Handle<Mesh> {
    if let Some(done) = cache.0.get(&source.id()) {
        return done.clone();
    }
    let Some(src) = meshes.get(source) else {
        warn!("cel: mesh {source:?} not loaded at scene ready; normals left as-is");
        return source.clone();
    };
    let handle = match simplify_normals(src) {
        Some(mesh) => meshes.add(mesh),
        None => source.clone(),
    };
    cache.0.insert(source.id(), handle.clone());
    handle
}

/// Returns a copy of `mesh` with normals blended toward outward directions from
/// the mesh's vertical center axis. The tallest bounding-box axis is taken as
/// vertical, so it works whatever the mesh's local up is. Skinning transforms
/// the new normals like the originals, so it holds up under animation.
fn simplify_normals(mesh: &Mesh) -> Option<Mesh> {
    let Some(VertexAttributeValues::Float32x3(pos)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION)
    else {
        return None;
    };
    let Some(VertexAttributeValues::Float32x3(nrm)) = mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
    else {
        return None;
    };
    let (min, max) = pos.iter().fold((Vec3::MAX, Vec3::MIN), |(lo, hi), p| {
        let p = Vec3::from(*p);
        (lo.min(p), hi.max(p))
    });
    let center = (min + max) / 2.0;
    let ext = max - min;
    let up = if ext.y >= ext.x && ext.y >= ext.z {
        Vec3::Y
    } else if ext.z >= ext.x {
        Vec3::Z
    } else {
        Vec3::X
    };
    let normals: Vec<[f32; 3]> = pos
        .iter()
        .zip(nrm)
        .map(|(p, n)| {
            let n = Vec3::from(*n);
            let radial = (Vec3::from(*p) - center).reject_from_normalized(up);
            let radial = radial.try_normalize().unwrap_or(n);
            n.lerp(radial, NORMAL_SIMPLIFY).try_normalize().unwrap_or(n).to_array()
        })
        .collect();
    let mut out = mesh.clone();
    out.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    Some(out)
}
