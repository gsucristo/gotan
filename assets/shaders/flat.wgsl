// Two-tone "cel" material (BTAS style): an extension over StandardMaterial.
//
// - form shading: a HARD step on N·L against the key directional light. A
//   fragment is either fully lit (base color) or in shadow (base * SHADOW_TINT).
//   No gradients — low-poly facets read as crisp painted shadow shapes.
// - cast shadows: casting is handled by the inherited StandardMaterial prepass.
//   Receiving uses the directional shadow map, only if this mesh is a shadow
//   receiver (`NotShadowReceiver` clears the bit). Cast shadow and form shadow
//   share the same tint, so they merge into one shape like painted animation.
// - emissive: added on top unshaded (lit windows, lamps); >1 feeds bloom.
// - paper: a static screen-space paper texture (fine tooth, slanted fibers,
//   soft mottling) multiplied over the paint, plus a faint whole-frame film
//   flicker at 24 fps.

#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    forward_io::{VertexOutput, FragmentOutput},
    mesh_view_bindings as view_bindings,
    mesh_types::MESH_FLAGS_SHADOW_RECEIVER_BIT,
    shadows,
}
#import boil::boil_noise

// Shadow color as a multiply on the base. Near-black with a cool blue bias
// (BTAS: shadows are dark graphic planes, keeping little of the base color),
// so light and shadow read as two separate painted shapes.
// Consts rather than uniforms so the extension carries no bindings of its own
// and never collides with StandardMaterial's (bindless) bind-group layout.
const SHADOW_TINT: vec3<f32> = vec3<f32>(0.05, 0.05, 0.11);
// N·L above this is lit. Well above 0 so anything not squarely facing the key
// falls into shadow: bigger, bolder shadow planes (BTAS reads ~half in shadow).
const TOON_THRESHOLD: f32 = 0.3;
// Paper texture amplitude as a fraction of the color, and film flicker
// amplitude. Multiplicative, so black stays black and the texture only shows
// where there is paint.
const PAPER_STRENGTH: f32 = 0.22;
const FLICKER_STRENGTH: f32 = 0.03;

// Integer PCG hash -> [0,1). Float sin/fract hashes band visibly at screen res.
fn hash(p: vec2<u32>, seed: u32) -> f32 {
    var v = p.x * 1973u + p.y * 9277u + seed * 26699u;
    v = v * 747796405u + 2891336453u;
    v = ((v >> ((v >> 28u) + 4u)) ^ v) * 277803737u;
    v = (v >> 22u) ^ v;
    return f32(v) / 4294967295.0;
}

// Paper texture in [0, 1] (mean ~0.5), from screen pixel coords so it stays
// fixed like the sheet the frame is painted on. Noise uses a constant seed,
// so it never animates.
fn paper(px: vec2<f32>) -> f32 {
    // Fine tooth: per-pixel speckle.
    let tooth = hash(vec2<u32>(px), 0u);
    // Fibers: noise stretched along a 30° slant.
    let r = vec2<f32>(px.x * 0.866 + px.y * 0.5, px.y * 0.866 - px.x * 0.5);
    let fiber = boil_noise(vec3<f32>(r.x * 0.02, r.y * 0.35, 0.0), 7u);
    // Mottling: soft low-frequency blotches of uneven absorption.
    let mottle = boil_noise(vec3<f32>(px * 0.012, 3.0), 11u) * 0.6
        + boil_noise(vec3<f32>(px * 0.05, 5.0), 13u) * 0.4;
    return tooth * 0.35 + fiber * 0.35 + mottle * 0.3;
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    // Reuse StandardMaterial's fragment machinery to get the base color, which
    // folds in vertex colors and alpha.
    let pbr_input = pbr_input_from_standard_material(in, is_front);
    let base = pbr_input.material.base_color;

    var lit = 1.0;
    if (view_bindings::lights.n_directional_lights > 0u) {
        let to_light = view_bindings::lights.directional_lights[0].direction_to_light;
        lit = step(TOON_THRESHOLD, dot(normalize(pbr_input.world_normal), to_light));

        if ((pbr_input.flags & MESH_FLAGS_SHADOW_RECEIVER_BIT) != 0u && lit > 0.0) {
            let view_z = dot(
                vec4<f32>(
                    view_bindings::view.view_from_world[0].z,
                    view_bindings::view.view_from_world[1].z,
                    view_bindings::view.view_from_world[2].z,
                    view_bindings::view.view_from_world[3].z,
                ),
                in.world_position,
            );
            // Hardware 2x2 filter can return fractions on the edge; snap to keep
            // the shadow edge a clean two-tone line.
            let s = shadows::fetch_directional_shadow(
                0u,
                in.world_position,
                pbr_input.world_normal,
                view_z,
                in.position.xy,
            );
            lit = step(0.5, s);
        }
    }

    let shade = mix(SHADOW_TINT, vec3<f32>(1.0), lit);
    let frame = u32(view_bindings::globals.time * 24.0);
    let flicker = 1.0 + (hash(vec2<u32>(0u), frame) - 0.5) * FLICKER_STRENGTH;
    let grain = (1.0 + (paper(in.position.xy) - 0.5) * PAPER_STRENGTH) * flicker;
    var out: FragmentOutput;
    out.color = vec4<f32>(
        (base.rgb * shade + pbr_input.material.emissive.rgb) * grain,
        base.a,
    );
    return out;
}
