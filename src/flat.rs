use bevy::gltf::GltfMaterialName;
use bevy::light::NotShadowReceiver;
use bevy::pbr::{ExtendedMaterial, MaterialExtension, MaterialPlugin};
use bevy::mesh::VertexAttributeValues;
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;


/// Two-tone cel material: StandardMaterial with our fragment shader swapped in.
/// See `assets/shaders/flat.wgsl`.
pub type FlatMaterial = ExtendedMaterial<StandardMaterial, FlatExtension>;

/// Per-part color overrides for the glTF character, keyed by its material name
/// (`GltfMaterialName`). Parts are untextured, so a part's color is purely its
/// material base color — set it here to retint that part. Leave empty to keep
/// the model's own colors. Known parts: Skin, Jacket, Pants, Shirt, Shoes, Hat,
/// HatBand, Hair, Eyes.
const PART_COLORS: &[(&str, Color)] = &[
    // Tango dancer: black suit + fedora, red shirt + hat band, tan skin.
    // "Black" cloth is painted as dark slate (BTAS convention) so the lit and
    // shadow tones stay distinct; true black is left to the shadow.
    ("Skin", Color::srgb(0.80, 0.56, 0.34)),
    ("Jacket", Color::srgb(0.20, 0.21, 0.27)),
    ("Pants", Color::srgb(0.20, 0.21, 0.27)),
    ("Shoes", Color::srgb(0.10, 0.10, 0.12)),
    ("Shirt", Color::srgb(0.62, 0.05, 0.05)),
    ("Hat", Color::srgb(0.16, 0.16, 0.21)),
    ("HatBand", Color::srgb(0.62, 0.05, 0.05)),
    ("Hair", Color::srgb(0.03, 0.03, 0.04)),
    ("Eyes", Color::srgb(0.02, 0.02, 0.02)),
];

/// Carries no bindings of its own — it only redirects the fragment shader. That
/// keeps it out of StandardMaterial's bindless bind-group layout (a plain
/// `#[uniform]` here fails pipeline validation). Shadow strength lives as a const
/// in the shader instead.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct FlatExtension {}

impl MaterialExtension for FlatExtension {
    fn fragment_shader() -> ShaderRef {
        "shaders/flat.wgsl".into()
    }
}

/// Builds a flat material with a solid base color and the given alpha mode.
pub fn flat(base_color: Color, alpha_mode: AlphaMode) -> FlatMaterial {
    ExtendedMaterial {
        base: StandardMaterial {
            base_color,
            alpha_mode,
            ..default()
        },
        extension: FlatExtension::default(),
    }
}

/// Unshaded glow (lit window, lamp): black base + emissive, which the shader adds
/// on top. Values above 1.0 feed bloom.
pub fn emissive(color: LinearRgba) -> FlatMaterial {
    ExtendedMaterial {
        base: StandardMaterial {
            base_color: Color::BLACK,
            emissive: color,
            ..default()
        },
        extension: FlatExtension::default(),
    }
}

/// How far character normals are bent toward a simple vertical-cylinder shape
/// (0 = original anatomy, 1 = pure cylinder). Shadow shapes then follow big
/// forms — one lit side, one dark side — instead of every muscle, like the
/// broad painted shadow planes in BTAS.
const NORMAL_SIMPLIFY: f32 = 0.55;

/// Returns a copy of `mesh` with normals blended toward outward directions from
/// the mesh's vertical center axis. The tallest bounding-box axis is taken as
/// vertical, so it works whatever the mesh's local up is. Skinning transforms
/// the new normals like the originals, so it holds up under animation.
pub fn simplify_normals(mesh: &Mesh) -> Option<Mesh> {
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

/// Keeps the `boil` shader import (shared by flat.wgsl and outline.wgsl) loaded
/// so `#import boil::...` resolves.
#[derive(Resource)]
struct BoilShader(#[allow(dead_code)] Handle<Shader>);

pub struct FlatMaterialPlugin;

impl Plugin for FlatMaterialPlugin {
    fn build(&self, app: &mut App) {
        let boil = app.world().resource::<AssetServer>().load("shaders/boil.wgsl");
        app.insert_resource(BoilShader(boil))
            .add_plugins(MaterialPlugin::<FlatMaterial>::default())
            .add_systems(Update, flatten_gltf_materials);
    }
}

/// The glTF loader colors the cowboy with per-part `StandardMaterial`s. Swap each
/// for a `FlatMaterial` (cloning the base so alpha/settings survive), applying any
/// `PART_COLORS` override for that part. The meshes stay shadow receivers *and*
/// casters (defaults), so the character self-shadows — hat over head, head over
/// body — while the flat shader keeps it gradient-free.
///
/// No `Added` filter: a mesh may spawn a frame before its material asset loads,
/// so we retry every frame and simply skip until the source material is ready.
/// Converted entities drop their `StandardMaterial` handle and stop matching.
fn flatten_gltf_materials(
    mut commands: Commands,
    meshes: Query<(Entity, &MeshMaterial3d<StandardMaterial>, Option<&GltfMaterialName>)>,
    std_materials: Res<Assets<StandardMaterial>>,
    mut flat_materials: ResMut<Assets<FlatMaterial>>,
) {
    for (entity, handle, mat_name) in &meshes {
        let Some(base) = std_materials.get(&handle.0) else {
            continue;
        };
        let mut base = base.clone();
        if let Some(name) = mat_name {
            if let Some((_, color)) = PART_COLORS.iter().find(|(n, _)| *n == name.0) {
                base.base_color = *color;
            }
        }
        let flat = flat_materials.add(ExtendedMaterial {
            base,
            extension: FlatExtension::default(),
        });
        // Whole character is cast-only: it throws its shadow onto the wall but
        // never receives (no self-shadow), so no acne on the low-poly geometry.
        commands
            .entity(entity)
            .remove::<MeshMaterial3d<StandardMaterial>>()
            .insert((MeshMaterial3d(flat), NotShadowReceiver));
    }
}
