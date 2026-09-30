// Inverted-hull ink outline (BTAS character linework).
//
// Drawn on a duplicate of each character mesh with FRONT faces culled: the
// hull's back faces, pushed outward along the (skinned) normal, peek out around
// the silhouette as a solid line. The push happens in clip space by a fixed
// number of pixels, so line weight stays constant regardless of zoom or model
// scale.
//
// Ink weight varies like a brush line: thick where the surface turns away from
// the key light (shadow side), thin on the lit side, and it boils (wanders
// slightly) each pose step. See boil.wgsl.

#import bevy_pbr::{
    mesh_functions,
    skinning,
    forward_io::Vertex,
    mesh_view_bindings::{view, globals},
    view_transformations::position_world_to_clip,
}
#import boil::{boil_seed, boil_noise}

// Width multipliers: shadow side vs lit side, and boil range (± fraction).
const INK_SHADOW: f32 = 1.6;
const INK_LIT: f32 = 0.55;
const INK_BOIL: f32 = 0.08;
const INK_BOIL_FREQ: f32 = 0.1;

struct Outline {
    color: vec4<f32>,
    // Line width in physical pixels.
    width_px: f32,
    // Unit direction toward the key light (world space). Passed in here since
    // the view's light bindings aren't visible to the vertex stage.
    to_light: vec3<f32>,
    // Character's current pose rate (Hz): the boil re-seeds in step with it.
    pose_rate: f32,
}
@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> outline: Outline;

struct VertexOut {
    @builtin(position) position: vec4<f32>,
}

@vertex
fn vertex(vertex: Vertex) -> VertexOut {
#ifdef SKINNED
    let world_from_local = skinning::skin_model(
        vertex.joint_indices,
        vertex.joint_weights,
        vertex.instance_index,
    );
    let world_normal = skinning::skin_normals(world_from_local, vertex.normal);
#else
    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);
    let world_normal = mesh_functions::mesh_normal_local_to_world(
        vertex.normal,
        vertex.instance_index,
    );
#endif

    let world_pos = mesh_functions::mesh_position_local_to_world(
        world_from_local,
        vec4<f32>(vertex.position, 1.0),
    );
    var clip = position_world_to_clip(world_pos.xyz);

    // Screen-space direction of the normal; push the vertex that way by
    // width_px. NDC spans 2 units across the viewport, and scaling by w keeps
    // it correct under perspective too (w = 1 for our orthographic camera).
    let n = normalize(world_normal);
    var width = outline.width_px;
    let facing = dot(n, outline.to_light);
    width *= mix(INK_SHADOW, INK_LIT, smoothstep(-0.2, 0.6, facing));
    let seed = boil_seed(globals.time, outline.pose_rate);
    width *= 1.0 + (boil_noise(world_pos.xyz * INK_BOIL_FREQ, seed) * 2.0 - 1.0) * INK_BOIL;

    let clip_n = (view.clip_from_world * vec4<f32>(n, 0.0)).xy;
    let len = length(clip_n);
    if (len > 1e-6) {
        let px_to_ndc = 2.0 / view.viewport.zw;
        clip = vec4<f32>(clip.xy + (clip_n / len) * width * px_to_ndc * clip.w, clip.zw);
    }

    var out: VertexOut;
    out.position = clip;
    return out;
}

@fragment
fn fragment() -> @location(0) vec4<f32> {
    return outline.color;
}
