//! "Black paper" dark-deco backdrop: red smog sky, silhouette skyline layers with
//! parallax, lit windows, sweeping searchlights, a fog band, and a near deco
//! facade that receives the character's cast shadow. Purely visual — nothing here
//! has a Collider, so it never touches physics.

use bevy::asset::RenderAssetUsages;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use crate::camera::{CAMERA_OFFSET, FRAME_LIFT};
use crate::flat::{self, FlatMaterial};

/// Horizontal/vertical scroll factor relative to the camera. 1.0 = moves with the
/// camera (infinitely far), 0.0 = fixed in the world. The camera is orthographic,
/// so depth alone gives no parallax; this system fakes it.
#[derive(Component)]
struct Parallax {
    factor: Vec2,
    base: Vec3,
}

/// Searchlight beam pivoting around its base.
#[derive(Component)]
struct Searchlight {
    speed: f32,
    phase: f32,
    sweep: f32,
}

pub struct BackdropPlugin;

impl Plugin for BackdropPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_backdrop).add_systems(
            Update,
            (
                apply_parallax.after(crate::camera::move_camera),
                sweep_searchlights,
            ),
        );
    }
}

/// Deterministic tiny PRNG so the skyline is stable run to run without a dep.
struct Lcg(u32);
impl Lcg {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.0 >> 8) as f32 / (1u32 << 24) as f32
    }
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.next()
    }
}

/// A flat plane colored by a vertical multi-stop gradient (vertex colors).
/// `stops` are (y, srgb color), bottom to top.
fn gradient_mesh(width: f32, stops: &[(f32, Color)]) -> Mesh {
    let hw = width / 2.0;
    let mut pos = Vec::new();
    let mut col = Vec::new();
    let mut idx = Vec::new();
    for (i, (y, c)) in stops.iter().enumerate() {
        let lin = c.to_linear().to_f32_array();
        pos.push([-hw, *y, 0.0]);
        pos.push([hw, *y, 0.0]);
        col.push(lin);
        col.push(lin);
        if i > 0 {
            let b = (i as u32 - 1) * 2;
            idx.extend_from_slice(&[b, b + 1, b + 3, b, b + 3, b + 2]);
        }
    }
    let n = pos.len();
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 0.0, 1.0]; n])
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, col)
        .with_inserted_indices(Indices::U32(idx))
}

/// Everything behind the facade is paint: no shadows either way.
fn paint() -> (NotShadowCaster, NotShadowReceiver) {
    (NotShadowCaster, NotShadowReceiver)
}

fn spawn_backdrop(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<FlatMaterial>>,
) {
    let rect = meshes.add(Rectangle::new(1.0, 1.0));
    let tri = meshes.add(Triangle2d::new(
        Vec2::new(-0.5, 0.0),
        Vec2::new(0.5, 0.0),
        Vec2::new(0.0, 1.0),
    ));

    // Note: the camera pitches ~10° down, so in this orthographic view anything
    // deeper appears higher on screen (~0.17 units up per unit of depth). Layer
    // y values below are tuned with that lift in mind.

    // --- Sky: locked to the camera. Near-black at the top, bleeding into a
    // blood-red / orange smog glow at the horizon.
    let sky = meshes.add(gradient_mesh(
        4000.0,
        &[
            (-400.0, Color::srgb(0.85, 0.30, 0.08)),
            (-250.0, Color::srgb(0.62, 0.12, 0.04)),
            (-120.0, Color::srgb(0.42, 0.07, 0.04)),
            (100.0, Color::srgb(0.13, 0.025, 0.035)),
            (350.0, Color::srgb(0.015, 0.01, 0.02)),
        ],
    ));
    let white = materials.add(flat::flat(Color::WHITE, AlphaMode::Opaque));
    spawn_layer(&mut commands, Vec2::ONE, Vec3::new(0.0, 0.0, -900.0), |p| {
        p.spawn((Mesh3d(sky), MeshMaterial3d(white), Transform::default(), paint()));
    });

    // --- Far skyline: dark maroon silhouettes against the glow, sparse dim windows.
    let far_body = materials.add(flat::flat(Color::srgb(0.20, 0.05, 0.06), AlphaMode::Opaque));
    let far_side = materials.add(flat::flat(Color::srgb(0.09, 0.02, 0.035), AlphaMode::Opaque));
    let far_win = materials.add(flat::emissive(LinearRgba::rgb(1.2, 0.55, 0.2)));
    let mut rng = Lcg(7);
    spawn_layer(&mut commands, Vec2::new(0.85, 0.8), Vec3::new(0.0, -330.0, -700.0), |p| {
        skyline(p, &mut rng, &rect, &tri, &far_body, &far_side, &far_win, 2600.0, (40.0, 110.0), (160.0, 400.0), 0.08, 1.0);
    });

    // --- Fog band drifting between the skylines.
    let fog = materials.add(flat::flat(Color::srgba(0.55, 0.16, 0.08, 0.22), AlphaMode::Blend));
    spawn_layer(&mut commands, Vec2::new(0.75, 0.7), Vec3::new(0.0, -210.0, -600.0), |p| {
        p.spawn((
            Mesh3d(rect.clone()),
            MeshMaterial3d(fog),
            Transform::from_scale(Vec3::new(4000.0, 90.0, 1.0)),
            paint(),
        ));
    });

    // --- Searchlights: additive beams raking the smog from behind the mid layer.
    let beam = meshes.add(Triangle2d::new(
        Vec2::new(0.0, 0.0),
        Vec2::new(-70.0, 1400.0),
        Vec2::new(70.0, 1400.0),
    ));
    // Additive: the cel shader doesn't premultiply, so the color *is* the added
    // light — keep it dim.
    let beam_mat = materials.add(flat::flat(Color::srgb(0.20, 0.16, 0.11), AlphaMode::Add));
    spawn_layer(&mut commands, Vec2::new(0.7, 0.65), Vec3::new(0.0, -300.0, -550.0), |p| {
        for (x, speed, phase) in [(-700.0, 0.23, 0.0), (250.0, 0.31, 2.0), (1100.0, 0.19, 4.0)] {
            p.spawn((
                Mesh3d(beam.clone()),
                MeshMaterial3d(beam_mat.clone()),
                Transform::from_xyz(x, 0.0, 0.0),
                Searchlight { speed, phase, sweep: 0.5 },
                paint(),
            ));
        }
    });

    // --- Mid skyline: near-black deco towers (setbacks + spires), warm windows.
    let mid_body = materials.add(flat::flat(Color::srgb(0.11, 0.05, 0.06), AlphaMode::Opaque));
    let mid_side = materials.add(flat::flat(Color::srgb(0.02, 0.012, 0.02), AlphaMode::Opaque));
    let mid_win = materials.add(flat::emissive(LinearRgba::rgb(2.4, 1.3, 0.45)));
    let mut rng = Lcg(42);
    spawn_layer(&mut commands, Vec2::new(0.6, 0.55), Vec3::new(0.0, -300.0, -400.0), |p| {
        skyline(p, &mut rng, &rect, &tri, &mid_body, &mid_side, &mid_win, 2400.0, (70.0, 150.0), (150.0, 360.0), 0.10, 1.6);
    });

    spawn_facade(&mut commands, &mut meshes, &mut materials);
}

/// Spawns a parallax layer root and fills it via `build`.
fn spawn_layer(
    commands: &mut Commands,
    factor: Vec2,
    base: Vec3,
    build: impl FnOnce(&mut ChildSpawnerCommands),
) {
    commands
        .spawn((
            Parallax { factor, base },
            Transform::from_translation(base),
            Visibility::default(),
        ))
        .with_children(build);
}

/// Share of each tower tier painted as its unlit (left) side.
const SIDE_FRACTION: f32 = 0.35;

/// A row of deco towers spanning `span` wide, bottoms at local y=0. Each tower is
/// 1–3 stacked, narrowing tiers (setbacks), optionally crowned by a spire, with a
/// sprinkle of lit windows.
#[allow(clippy::too_many_arguments)]
fn skyline(
    p: &mut ChildSpawnerCommands,
    rng: &mut Lcg,
    rect: &Handle<Mesh>,
    tri: &Handle<Mesh>,
    body: &Handle<FlatMaterial>,
    side: &Handle<FlatMaterial>,
    window: &Handle<FlatMaterial>,
    span: f32,
    width: (f32, f32),
    height: (f32, f32),
    lit_chance: f32,
    win_size: f32,
) {
    let mut x = -span;
    while x < span {
        let w = rng.range(width.0, width.1);
        let h = rng.range(height.0, height.1);
        let cx = x + w / 2.0;
        let mut tier_w = w;
        let mut y = 0.0;
        let mut tier_h = h;
        let tiers = 1 + (rng.next() * 3.0) as u32;
        for t in 0..tiers {
            if t > 0 {
                tier_w *= rng.range(0.55, 0.8);
                tier_h = h * rng.range(0.15, 0.3);
            }
            p.spawn((
                Mesh3d(rect.clone()),
                MeshMaterial3d(body.clone()),
                Transform::from_xyz(cx, y + tier_h / 2.0, 0.0)
                    .with_scale(Vec3::new(tier_w, tier_h, 1.0)),
                paint(),
            ));
            // Painted shadow side (key light is from the right): the towers are
            // flat cards, so fake the unlit side plane as a dark strip.
            let side_w = tier_w * SIDE_FRACTION;
            p.spawn((
                Mesh3d(rect.clone()),
                MeshMaterial3d(side.clone()),
                Transform::from_xyz(cx - tier_w / 2.0 + side_w / 2.0, y + tier_h / 2.0, 0.1)
                    .with_scale(Vec3::new(side_w, tier_h, 1.0)),
                paint(),
            ));
            // Windows: grid over this tier, only a few lit.
            let (sx, sy) = (9.0 * win_size, 14.0 * win_size);
            let cols = ((tier_w - 8.0) / sx).floor() as i32;
            let rows = ((tier_h - 10.0) / sy).floor() as i32;
            for r in 0..rows {
                for c in 0..cols {
                    if rng.next() > lit_chance {
                        continue;
                    }
                    let wx = cx - (cols as f32 - 1.0) * sx / 2.0 + c as f32 * sx;
                    let wy = y + 10.0 + r as f32 * sy + sy / 2.0;
                    p.spawn((
                        Mesh3d(rect.clone()),
                        MeshMaterial3d(window.clone()),
                        Transform::from_xyz(wx, wy, 0.5)
                            .with_scale(Vec3::new(3.0 * win_size, 5.0 * win_size, 1.0)),
                        paint(),
                    ));
                }
            }
            y += tier_h;
        }
        if rng.next() < 0.35 {
            let sw = tier_w * rng.range(0.25, 0.5);
            let sh = rng.range(40.0, 140.0);
            p.spawn((
                Mesh3d(tri.clone()),
                MeshMaterial3d(body.clone()),
                Transform::from_xyz(cx, y, 0.0).with_scale(Vec3::new(sw, sh, 1.0)),
                paint(),
            ));
        }
        x += w + rng.range(0.0, 140.0);
    }
}

/// Street level: the ground platform's bottom edge (see `setup` in main.rs).
/// Below it is the building we're standing on — a black mass, black paper.
const STREET_Y: f32 = -180.0;

/// Front face of the facade = the platforms' back face (z = -24), so platforms sit
/// flush against it and the key light throws the character's shadow onto it.
const FACADE_FRONT_Z: f32 = -24.0;

/// Near building facade the gameplay runs along: warm-dark stone blocks with
/// stepped deco tops. Protruding pilasters catch the key light on their front and
/// fall into cel shadow on their left faces — cheap relief.
fn spawn_facade(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<FlatMaterial>,
) {
    let stone = materials.add(flat::flat(Color::srgb(0.50, 0.33, 0.25), AlphaMode::Opaque));
    let trim = materials.add(flat::flat(Color::srgb(0.60, 0.40, 0.28), AlphaMode::Opaque));
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));

    // The rooftop we run on: occludes sky and skyline below street level so the
    // bottom of the frame is near-black silhouette. Spans the gameplay depth,
    // just behind the platforms' front face.
    let mass = materials.add(flat::flat(Color::srgb(0.03, 0.02, 0.025), AlphaMode::Opaque));
    commands.spawn((
        Mesh3d(cube.clone()),
        MeshMaterial3d(mass),
        Transform::from_xyz(0.0, STREET_Y - 1000.0, -8.0)
            .with_scale(Vec3::new(5000.0, 2000.0, 32.0)),
        NotShadowReceiver,
    ));

    let bottom = STREET_Y;
    let depth = 20.0;

    // (center x, width, top y) — tops vary so the skyline shows through above.
    // Gaps between blocks let the skyline reach down to the street.
    let blocks = [
        (-1180.0, 260.0, -60.0),
        (-900.0, 200.0, 60.0),
        (-560.0, 320.0, -20.0),
        (-180.0, 200.0, 90.0),
        (120.0, 260.0, -80.0),
        (470.0, 240.0, 30.0),
        (820.0, 300.0, -40.0),
        (1180.0, 240.0, 80.0),
    ];
    for (cx, w, top) in blocks {
        let h = top - bottom;
        commands.spawn((
            Mesh3d(cube.clone()),
            MeshMaterial3d(stone.clone()),
            Transform::from_xyz(cx, bottom + h / 2.0, FACADE_FRONT_Z - depth / 2.0)
                .with_scale(Vec3::new(w, h, depth)),
        ));
        // Stepped crown: two narrowing setbacks.
        let mut sw = w;
        let mut y = top;
        for step_h in [18.0, 12.0] {
            sw *= 0.7;
            commands.spawn((
                Mesh3d(cube.clone()),
                MeshMaterial3d(stone.clone()),
                Transform::from_xyz(cx, y + step_h / 2.0, FACADE_FRONT_Z - depth / 2.0)
                    .with_scale(Vec3::new(sw, step_h, depth)),
            ));
            y += step_h;
        }
        // Cornice band just under the top.
        commands.spawn((
            Mesh3d(cube.clone()),
            MeshMaterial3d(trim.clone()),
            Transform::from_xyz(cx, top - 14.0, FACADE_FRONT_Z + 3.0)
                .with_scale(Vec3::new(w + 8.0, 10.0, 6.0)),
        ));
        // Pilasters.
        let n = (w / 70.0).floor() as i32;
        for i in 0..=n {
            let px = cx - w / 2.0 + 12.0 + i as f32 * (w - 24.0) / n.max(1) as f32;
            commands.spawn((
                Mesh3d(cube.clone()),
                MeshMaterial3d(trim.clone()),
                Transform::from_xyz(px, bottom + (h - 24.0) / 2.0, FACADE_FRONT_Z + 2.0)
                    .with_scale(Vec3::new(10.0, h - 24.0, 4.0)),
            ));
        }
    }
}

/// Layers follow the camera's displacement from its start by their factor.
fn apply_parallax(
    camera: Single<&Transform, (With<Camera3d>, Without<Parallax>)>,
    mut layers: Query<(&Parallax, &mut Transform)>,
) {
    let cam = Vec2::new(camera.translation.x, camera.translation.y - CAMERA_OFFSET.y - FRAME_LIFT);
    for (layer, mut tf) in &mut layers {
        tf.translation.x = layer.base.x + cam.x * layer.factor.x;
        tf.translation.y = layer.base.y + cam.y * layer.factor.y;
    }
}

fn sweep_searchlights(time: Res<Time>, mut beams: Query<(&Searchlight, &mut Transform)>) {
    let t = time.elapsed_secs();
    for (s, mut tf) in &mut beams {
        tf.rotation = Quat::from_rotation_z((t * s.speed + s.phase).sin() * s.sweep);
    }
}
