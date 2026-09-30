use bevy::prelude::*;

use crate::player::Player;

/// Camera position relative to what it looks at. Elevated + pulled back on +Z so
/// the fixed orthographic view tilts ~10° down (atan(85/480)), revealing platform
/// tops while staying a faked-2D look. Shared with `setup` so the spawn transform
/// and the follow target agree.
pub const CAMERA_OFFSET: Vec3 = Vec3::new(0.0, 85.0, 480.0);

/// World units the view is raised above the player, so the player sits in the
/// lower third and the skyline gets the frame. Pure translation (the tilt comes
/// from `CAMERA_OFFSET`'s direction), so it's applied to the follow target too.
pub const FRAME_LIFT: f32 = 110.0;

const FOLLOW_STRENGTH: f32 = 8.0;

pub fn move_camera(
    time: Res<Time>,
    player: Single<&Transform, (With<Player>, Without<Camera3d>)>,
    mut camera: Single<&mut Transform, (With<Camera3d>, Without<Player>)>,
) {
    let dt = time.delta_secs();
    // Follow on XY; depth stays fixed and rotation is left as spawned, so the
    // camera never rotates — only glides to keep the player framed.
    let target = Vec3::new(
        player.translation.x + CAMERA_OFFSET.x,
        player.translation.y + CAMERA_OFFSET.y + FRAME_LIFT,
        CAMERA_OFFSET.z,
    );
    let t = 1.0 - (-FOLLOW_STRENGTH * dt).exp();
    camera.translation = camera.translation.lerp(target, t);
}
