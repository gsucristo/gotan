mod camera;
mod character;
mod debug;
mod input;
mod interpolation;
mod physics;
mod player;

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
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let player_pos = Vec3::new(0.0, 40.0, 0.0);

    // High-res shadow map: more texels for the same cascade → crisper hard edge.
    commands.insert_resource(DirectionalLightShadowMap { size: 8192 });

    // Fixed orthographic 3D camera. Orthographic keeps the faked-2D look (no
    // perspective shrink with depth); the elevated offset gives a ~10° down-tilt.
    commands.spawn((
        Camera3d::default(),
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::FixedVertical {
                viewport_height: 540.0,
            },
            // Wide range: the camera sits ~480 units back, so the default
            // near=0/far=1000 would clip the scene. Symmetric range is safe.
            near: -2000.0,
            far: 2000.0,
            ..OrthographicProjection::default_3d()
        }),
        Transform::from_translation(player_pos + camera::CAMERA_OFFSET)
            .looking_at(player_pos, Vec3::Y),
        // Per-camera ambient fill. Kept low so the cast shadow stays legible.
        AmbientLight {
            brightness: 200.0,
            ..default()
        },
        // Sharpest filter → hard-edged shadow (default Gaussian softens it).
        ShadowFilteringMethod::Hardware2x2,
    ));

    // Backdrop wall a bit behind the gameplay plane (z=0), facing the camera, so
    // the grazing light throws the ball's long shadow onto it. Purely visual: no
    // Collider/Solid, so it never touches physics.
    // Front face at z=-24 = the platforms' back face (depth 48 centered on z=0),
    // so platforms sit flush against it. Depth 20 → center at -34.
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(4000.0, 2000.0, 20.0))),
        MeshMaterial3d(materials.add(Color::srgb(0.82, 0.80, 0.78))),
        Transform::from_xyz(0.0, 200.0, -34.0),
    ));

    // Key light aimed mostly INTO the wall (-Z dominant) with only a slight down
    // and sideways lean, so the sphere throws its shadow straight back onto the
    // wall behind it (not down onto the floor). Shadow maps enabled.
    commands.spawn((
        DirectionalLight {
            illuminance: 6000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::default().looking_to(Vec3::new(-0.28, -0.22, -0.93), Vec3::Y),
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

    // Sphere radius matches the 16-unit collider half-extent. Collision stays AABB.
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

    let solid = Color::srgb(0.45, 0.45, 0.5);
    let one_way = Color::srgba(0.3, 0.7, 0.9, 0.55);
    spawn_platform(&mut commands, &mut meshes, &mut materials, 0.0, -160.0, 1600.0, 40.0, false, solid);
    spawn_platform(&mut commands, &mut meshes, &mut materials, 300.0, 20.0, 180.0, 24.0, false, solid);
    spawn_platform(&mut commands, &mut meshes, &mut materials, -320.0, 110.0, 240.0, 24.0, false, solid);
    spawn_platform(&mut commands, &mut meshes, &mut materials, -80.0, 70.0, 160.0, 12.0, true, one_way);
    spawn_platform(&mut commands, &mut meshes, &mut materials, 180.0, 150.0, 160.0, 12.0, true, one_way);
}

fn spawn_platform(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    one_way: bool,
    color: Color,
) {
    let material = StandardMaterial {
        base_color: color,
        // One-way platforms keep their translucent look.
        alpha_mode: if one_way {
            AlphaMode::Blend
        } else {
            AlphaMode::Opaque
        },
        ..default()
    };
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(w, h, PLATFORM_DEPTH))),
        MeshMaterial3d(materials.add(material)),
        Transform::from_xyz(x, y, 0.0),
        Solid,
        Collider {
            half: Vec2::new(w / 2.0, h / 2.0),
            one_way,
        },
    ));
}