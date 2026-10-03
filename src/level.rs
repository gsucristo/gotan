//! The level: platform layout and the player's spawn. Physics is purely 2D (XY
//! plane at z=0); platforms get `PLATFORM_DEPTH` only as a render cue that the
//! slight camera tilt reveals as tops.

use bevy::prelude::*;

use crate::cel::{self, FlatMaterial};
use crate::physics::StaticBody;
use crate::player;

struct Platform {
    center: Vec2,
    size: Vec2,
    one_way: bool,
}

const fn solid(x: f32, y: f32, w: f32, h: f32) -> Platform {
    Platform {
        center: Vec2::new(x, y),
        size: Vec2::new(w, h),
        one_way: false,
    }
}

const fn ledge(x: f32, y: f32, w: f32, h: f32) -> Platform {
    Platform {
        one_way: true,
        ..solid(x, y, w, h)
    }
}

/// The rooftop we run on.
const GROUND: Platform = solid(0.0, -160.0, 1600.0, 40.0);

/// Where feet rest at street level.
pub const GROUND_TOP: f32 = GROUND.center.y + GROUND.size.y / 2.0;
/// The ground platform's bottom edge. Below it is the building we're standing
/// on — a black mass, black paper (see backdrop.rs).
pub const STREET_Y: f32 = GROUND.center.y - GROUND.size.y / 2.0;

pub const PLATFORM_DEPTH: f32 = 48.0;

const PLATFORMS: [Platform; 5] = [
    GROUND,
    solid(300.0, 20.0, 180.0, 24.0),
    solid(-320.0, 110.0, 240.0, 24.0),
    ledge(-80.0, 70.0, 160.0, 12.0),
    ledge(180.0, 150.0, 160.0, 12.0),
];

/// Player's feet at spawn.
const PLAYER_SPAWN: Vec2 = Vec2::new(0.0, 24.0);

const SOLID_COLOR: Color = Color::srgb(0.14, 0.16, 0.22);
/// One-way platforms keep a translucent look.
const LEDGE_COLOR: Color = Color::srgba(0.2, 0.42, 0.6, 0.5);

pub struct LevelPlugin;

impl Plugin for LevelPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_level);
    }
}

fn spawn_level(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<FlatMaterial>>,
) {
    let solid = materials.add(cel::flat(SOLID_COLOR, AlphaMode::Opaque));
    let ledge = materials.add(cel::flat(LEDGE_COLOR, AlphaMode::Blend));
    for p in &PLATFORMS {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(p.size.x, p.size.y, PLATFORM_DEPTH))),
            MeshMaterial3d(if p.one_way { ledge.clone() } else { solid.clone() }),
            Transform::from_translation(p.center.extend(0.0)),
            StaticBody {
                size: p.size,
                one_way: p.one_way,
            },
        ));
    }
    player::spawn(&mut commands, &assets, PLAYER_SPAWN);
}
