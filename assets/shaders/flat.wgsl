// Flat "fake-2D" material: an extension over StandardMaterial.
//
// - form shadows OFF: the surface renders at its flat base color regardless of
//   how its normal faces the light (no diffuse/NdotL gradient).
// - cast shadows ON: casting is handled by the inherited StandardMaterial
//   prepass, so meshes still throw shadows onto the wall.
// - self-shadow control: we only darken where the directional shadow map says
//   the fragment is occluded, AND only if this mesh is a shadow receiver. Tag a
//   mesh with `NotShadowReceiver` to clear that bit -> it never shadows itself
//   while still casting onto everything else.

#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    forward_io::{VertexOutput, FragmentOutput},
    mesh_view_bindings as view_bindings,
    mesh_types::MESH_FLAGS_SHADOW_RECEIVER_BIT,
    shadows,
}

// How dark a cast shadow is (0 = black, 1 = invisible). A const rather than a
// uniform so the extension carries no bindings of its own and never collides
// with StandardMaterial's (bindless) bind-group layout.
const SHADOW_STRENGTH: f32 = 0.08;

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    // Reuse StandardMaterial's fragment machinery to get the base color, which
    // folds in vertex colors (how the low-poly cowboy is tinted) and alpha.
    let pbr_input = pbr_input_from_standard_material(in, is_front);
    let base = pbr_input.material.base_color;

    // How lit this fragment is: 1.0 unless a cast shadow falls on it. We
    // deliberately never touch the surface normal for shading -> flat look.
    var lit = 1.0;
    if ((pbr_input.flags & MESH_FLAGS_SHADOW_RECEIVER_BIT) != 0u
            && view_bindings::lights.n_directional_lights > 0u) {
        let view_z = dot(
            vec4<f32>(
                view_bindings::view.view_from_world[0].z,
                view_bindings::view.view_from_world[1].z,
                view_bindings::view.view_from_world[2].z,
                view_bindings::view.view_from_world[3].z,
            ),
            in.world_position,
        );
        lit = shadows::fetch_directional_shadow(
            0u,
            in.world_position,
            pbr_input.world_normal,
            view_z,
            in.position.xy,
        );
    }

    let shade = mix(SHADOW_STRENGTH, 1.0, lit);
    var out: FragmentOutput;
    out.color = vec4<f32>(base.rgb * shade, base.a);
    return out;
}
