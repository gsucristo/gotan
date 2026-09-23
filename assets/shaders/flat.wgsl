// Flat "fake-2D" material: an extension over StandardMaterial.
//
// - form shadows OFF: the surface renders at its flat base color regardless of
//   how its normal faces the light (no diffuse/NdotL gradient).
// - cast shadows ON: casting is handled by the inherited StandardMaterial
//   prepass, so meshes still throw shadows onto the wall.
// - self-shadow control: we only darken where a SPOT-light shadow map says the
//   fragment is occluded, AND only if this mesh is a shadow receiver. Tag a mesh
//   with `NotShadowReceiver` to clear that bit -> it never shadows itself while
//   still casting onto everything else.
//
// We sample a spot light (a nearby point-source) rather than a directional light
// so the cast shadow is MAGNIFIED — projected larger than the caster onto the
// wall. Spot lights are clustered, so we find the ones covering this fragment's
// cluster and take the darkest (min) shadow.

#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    forward_io::{VertexOutput, FragmentOutput},
    mesh_view_bindings as view_bindings,
    mesh_view_types,
    mesh_types::MESH_FLAGS_SHADOW_RECEIVER_BIT,
    clustered_forward as clustering,
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
    if ((pbr_input.flags & MESH_FLAGS_SHADOW_RECEIVER_BIT) != 0u) {
        // Which light cluster this fragment falls in (needs view-space depth).
        let view_z = dot(
            vec4<f32>(
                view_bindings::view.view_from_world[0].z,
                view_bindings::view.view_from_world[1].z,
                view_bindings::view.view_from_world[2].z,
                view_bindings::view.view_from_world[3].z,
            ),
            in.world_position,
        );
        let cluster_index = clustering::view_fragment_cluster_index(
            in.position.xy,
            view_z,
            pbr_input.is_orthographic,
        );
        let ranges = clustering::unpack_clusterable_object_index_ranges(cluster_index);

        // Take the darkest shadow among the spot lights covering this cluster.
        for (var i = ranges.first_spot_light_index_offset;
                i < ranges.first_reflection_probe_index_offset;
                i = i + 1u) {
            let light_id = clustering::get_clusterable_object_id(i);
            let light = &view_bindings::clustered_lights.data[light_id];
            if (((*light).flags & mesh_view_types::POINT_LIGHT_FLAGS_SHADOWS_ENABLED_BIT) != 0u) {
                lit = min(lit, shadows::fetch_spot_shadow(
                    light_id,
                    in.world_position,
                    pbr_input.world_normal,
                    (*light).shadow_map_near_z,
                    in.position.xy,
                ));
            }
        }
    }

    let shade = mix(SHADOW_STRENGTH, 1.0, lit);
    var out: FragmentOutput;
    out.color = vec4<f32>(base.rgb * shade, base.a);
    return out;
}
