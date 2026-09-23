use bevy::gltf::GltfMaterialName;
use bevy::pbr::{ExtendedMaterial, MaterialExtension, MaterialPlugin};
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;

/// Flat "fake-2D" material: StandardMaterial with our fragment shader swapped in.
/// See `assets/shaders/flat.wgsl`.
pub type FlatMaterial = ExtendedMaterial<StandardMaterial, FlatExtension>;

/// Per-part color overrides for the glTF character, keyed by its material name
/// (`GltfMaterialName`). The cowboy has no vertex colors, so a part's color is
/// purely its material base color — set it here to retint that part. This is the
/// single place to recolor the proof-of-concept model; leave empty to keep the
/// model's original colors. Known parts: Skin, Face, Jacket, Top, Scarf, Pants,
/// HatBrown, HatLightBrown, Hair.
const PART_COLORS: &[(&str, Color)] = &[
    // Tango dancer: black suit + hat, red collar accent, tan skin.
    ("Skin", Color::srgb(0.80, 0.56, 0.34)),
    ("Face", Color::srgb(0.80, 0.56, 0.34)),
    ("Jacket", Color::srgb(0.06, 0.06, 0.07)),
    ("Top", Color::srgb(0.06, 0.06, 0.07)),
    ("Pants", Color::srgb(0.06, 0.06, 0.07)),
    ("Scarf", Color::srgb(0.58, 0.05, 0.05)),
    ("HatBrown", Color::srgb(0.03, 0.03, 0.04)),
    ("HatLightBrown", Color::srgb(0.58, 0.05, 0.05)),
    ("Hair", Color::srgb(0.04, 0.04, 0.05)),
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

pub struct FlatMaterialPlugin;

impl Plugin for FlatMaterialPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<FlatMaterial>::default())
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
        commands
            .entity(entity)
            .remove::<MeshMaterial3d<StandardMaterial>>()
            .insert(MeshMaterial3d(flat));
    }
}
