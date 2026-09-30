use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::post_process::bloom::Bloom;
use bevy::post_process::effect_stack::Vignette;
use bevy::prelude::*;

/// Camera components for the cel look: exact flat colors (no tonemapping curve
/// desaturating the palette), bloom only on emissive lights, dark vignette.
/// Paper texture + film flicker live in the cel shader (`flat.wgsl`) rather
/// than a post pass, so they apply multiplicatively in the material's own color
/// space.
pub fn camera_bundle() -> impl Bundle {
    (
        Tonemapping::None,
        Bloom {
            intensity: 0.25,
            ..Bloom::NATURAL
        },
        Vignette {
            intensity: 0.7,
            radius: 0.85,
            smoothness: 2.5,
            ..default()
        },
    )
}
