use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;

/// Two-tone cel material: StandardMaterial with our fragment shader swapped in.
/// See `assets/shaders/flat.wgsl`.
pub type FlatMaterial = ExtendedMaterial<StandardMaterial, FlatExtension>;

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

/// Wraps an existing StandardMaterial (alpha, emissive and the rest survive).
pub fn from_standard(base: StandardMaterial) -> FlatMaterial {
    ExtendedMaterial {
        base,
        extension: FlatExtension::default(),
    }
}

/// Builds a flat material with a solid base color and the given alpha mode.
pub fn flat(base_color: Color, alpha_mode: AlphaMode) -> FlatMaterial {
    from_standard(StandardMaterial {
        base_color,
        alpha_mode,
        ..default()
    })
}

/// Unshaded glow (lit window, lamp): black base + emissive, which the shader adds
/// on top. Values above 1.0 feed bloom.
pub fn emissive(color: LinearRgba) -> FlatMaterial {
    from_standard(StandardMaterial {
        base_color: Color::BLACK,
        emissive: color,
        ..default()
    })
}
