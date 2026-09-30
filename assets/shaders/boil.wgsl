// "Boil": hand-drawn line jitter, used by the ink outline (outline.wgsl).
// boil_noise with a fixed seed also serves as static noise (flat.wgsl paper). Each new cel is redrawn by hand, so edges
// wander slightly from frame to frame. We fake it with low-frequency noise
// re-seeded at the character's current pose rate (passed in by the caller from
// character.rs `PoseRate`). Both step on floor(globals.time * rate), so edges
// hold still between poses and shift on exactly the frame the pose steps.
#define_import_path boil

fn boil_seed(time: f32, rate: f32) -> u32 {
    return u32(time * rate);
}

fn hash3(p: vec3<i32>, seed: u32) -> f32 {
    var v = bitcast<u32>(p.x) * 1973u + bitcast<u32>(p.y) * 9277u
        + bitcast<u32>(p.z) * 26699u + seed * 104729u;
    v = v * 747796405u + 2891336453u;
    v = ((v >> ((v >> 28u) + 4u)) ^ v) * 277803737u;
    v = (v >> 22u) ^ v;
    return f32(v) / 4294967295.0;
}

// Smooth trilinear value noise in [0, 1].
fn boil_noise(p: vec3<f32>, seed: u32) -> f32 {
    let i = vec3<i32>(floor(p));
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let x00 = mix(hash3(i + vec3(0, 0, 0), seed), hash3(i + vec3(1, 0, 0), seed), u.x);
    let x10 = mix(hash3(i + vec3(0, 1, 0), seed), hash3(i + vec3(1, 1, 0), seed), u.x);
    let x01 = mix(hash3(i + vec3(0, 0, 1), seed), hash3(i + vec3(1, 0, 1), seed), u.x);
    let x11 = mix(hash3(i + vec3(0, 1, 1), seed), hash3(i + vec3(1, 1, 1), seed), u.x);
    return mix(mix(x00, x10, u.y), mix(x01, x11, u.y), u.z);
}

// Signed variant in [-1, 1].
fn boil_signed(p: vec3<f32>, seed: u32) -> f32 {
    return boil_noise(p, seed) * 2.0 - 1.0;
}
