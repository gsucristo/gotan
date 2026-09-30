mod backdrop;
mod camera;
mod character;
mod debug;
mod flat;
mod input;
mod interpolation;
mod outline;
mod physics;
mod player;
mod post;

use bevy::asset::AssetPlugin;
use bevy::camera::{OrthographicProjection, Projection, ScalingMode};
use bevy::light::{CascadeShadowConfigBuilder, DirectionalLightShadowMap, ShadowFilteringMethod};
use bevy::prelude::*;
use bevy::window::{PresentMode, Window, WindowPlugin};

use physics::{Collider, Solid, Velocity};
use player::Player;

pub const PHYSICS_TICK_HZ: f64 = 240.0;

#[derive(States, Debug, Default, Clone, PartialEq, Eq, Hash)]
enum GameState {
    #[default]
    Playing,
}

fn main() -> AppExit {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "bevy-platformer".into(),
                        present_mode: PresentMode::AutoNoVsync,
                        ..default()
                    }),
                    ..default()
                })
                .set(AssetPlugin {
                    watch_for_changes_override: Some(true),
                    ..default()
                }),
        )
        .add_plugins(bevy::diagnostic::FrameTimeDiagnosticsPlugin::default())
        .add_plugins(debug::DebugPlugin)
        .add_plugins(character::CharacterPlugin)
        .add_plugins(flat::FlatMaterialPlugin)
        .add_plugins(backdrop::BackdropPlugin)
        .add_plugins(outline::OutlinePlugin)
        .insert_resource(Time::<Fixed>::from_hz(PHYSICS_TICK_HZ))
        .init_resource::<input::MoveIntent>()
        .init_state::<GameState>()
        .add_systems(Startup, setup)
        .add_systems(
            FixedUpdate,
            (
                interpolation::capture_prev,
                input::read_input,
                player::apply_movement,
                physics::apply_gravity,
                physics::integrate_and_collide,
                interpolation::store_current,
            )
                .chain()
                .run_if(in_state(GameState::Playing)),
        )
        .add_systems(
            Update,
            (
                (interpolation::update_interpolation, camera::move_camera).chain(),
            )
                .run_if(in_state(GameState::Playing)),
        )
        .run()
}

/// Z-depth of the platform meshes. Physics is purely 2D (XY plane at z=0); the
/// depth is only a render cue that the slight camera tilt reveals as tops.
const PLATFORM_DEPTH: f32 = 48.0;

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<flat::FlatMaterial>>,
) {
    let player_pos = Vec3::new(0.0, 40.0, 0.0);

    // High-res shadow map: more texels for the same cascade → crisper hard edge.
    commands.insert_resource(DirectionalLightShadowMap { size: 8192 });
    // Black paper: everything is painted light-onto-dark (see backdrop.rs).
    commands.insert_resource(ClearColor(Color::BLACK));

    // Fixed orthographic 3D camera. Orthographic keeps the faked-2D look (no
    // perspective shrink with depth); the elevated offset gives a ~10° down-tilt.
    commands.spawn((
        Camera3d::default(),
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::FixedVertical {
                viewport_height: 440.0,
            },
            // Wide range: the camera sits ~480 units back, so the default
            // near=0/far=1000 would clip the scene. Symmetric range is safe.
            near: -2000.0,
            far: 2000.0,
            ..OrthographicProjection::default_3d()
        }),
        Transform::from_translation(player_pos + camera::CAMERA_OFFSET)
            .looking_at(player_pos, Vec3::Y)
            .with_translation(player_pos + camera::CAMERA_OFFSET + Vec3::Y * camera::FRAME_LIFT),
        // Per-camera ambient fill. Kept low so the cast shadow stays legible.
        AmbientLight {
            brightness: 200.0,
            ..default()
        },
        // Sharpest filter → hard-edged shadow (default Gaussian softens it).
        ShadowFilteringMethod::Hardware2x2,
        post::camera_bundle(),
    ));

    // Key light rakes across from the right (shadow thrown left) and into the wall
    // (-Z). Strongly side-on so forms split roughly half lit / half shadow — the
    // BTAS two-tone read — and the cast shadow stretches long and diagonal.
    // Directional (parallel), so the shadow is caster-sized.
    commands.spawn((
        DirectionalLight {
            illuminance: 6000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::default().looking_to(Vec3::new(-0.8, -0.25, -0.55), Vec3::Y),
        // One cascade tightly bounded to the visible scene, so all shadow-map
        // texels pack onto what's on screen → small texels → clean hard edge.
        CascadeShadowConfigBuilder {
            num_cascades: 1,
            minimum_distance: 0.1,
            maximum_distance: 1200.0,
            first_cascade_far_bound: 1200.0,
            overlap_proportion: 0.0,
        }
        .build(),
    ));

    // Player is now an invisible physics body; the animated cowboy rides it as a
    // child (see character::spawn_model). Visibility on the body lets the child
    // scene inherit visibility.
    let player = commands
        .spawn((
            Player::default(),
            Transform::from_translation(player_pos),
            Visibility::default(),
            Velocity::default(),
            Collider {
                half: Vec2::new(16.0, 16.0),
                one_way: false,
            },
            interpolation::Interpolated::new(player_pos),
        ))
        .id();
    character::spawn_model(&mut commands, player, &asset_server);

    let solid = Color::srgb(0.14, 0.16, 0.22);
    let one_way = Color::srgba(0.2, 0.42, 0.6, 0.5);
    spawn_platform(&mut commands, &mut meshes, &mut materials, 0.0, -160.0, 1600.0, 40.0, false, solid);
    spawn_platform(&mut commands, &mut meshes, &mut materials, 300.0, 20.0, 180.0, 24.0, false, solid);
    spawn_platform(&mut commands, &mut meshes, &mut materials, -320.0, 110.0, 240.0, 24.0, false, solid);
    spawn_platform(&mut commands, &mut meshes, &mut materials, -80.0, 70.0, 160.0, 12.0, true, one_way);
    spawn_platform(&mut commands, &mut meshes, &mut materials, 180.0, 150.0, 160.0, 12.0, true, one_way);
}

fn spawn_platform(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<flat::FlatMaterial>,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    one_way: bool,
    color: Color,
) {
    // One-way platforms keep their translucent look.
    let alpha_mode = if one_way {
        AlphaMode::Blend
    } else {
        AlphaMode::Opaque
    };
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(w, h, PLATFORM_DEPTH))),
        MeshMaterial3d(materials.add(flat::flat(color, alpha_mode))),
        Transform::from_xyz(x, y, 0.0),
        Solid,
        Collider {
            half: Vec2::new(w / 2.0, h / 2.0),
            one_way,
        },
    ));
}