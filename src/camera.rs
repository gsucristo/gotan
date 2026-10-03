//! Fixed orthographic 3D camera that glides after a `CameraTarget`.

use bevy::camera::{OrthographicProjection, Projection, ScalingMode};
use bevy::prelude::*;

use crate::cel;

/// Camera position relative to its focus. Elevated + pulled back on +Z so the
/// fixed orthographic view tilts ~10° down (atan(85/480)), revealing platform
/// tops while staying a faked-2D look. Orthographic keeps the 2D feel (no
/// perspective shrink with depth).
const CAMERA_OFFSET: Vec3 = Vec3::new(0.0, 85.0, 480.0);

/// World units the view is raised above the focus, so the target sits in the
/// lower third and the skyline gets the frame. Pure translation (the tilt comes
/// from `CAMERA_OFFSET`'s direction).
const FRAME_LIFT: f32 = 126.0;

const FOLLOW_STRENGTH: f32 = 8.0;

/// What the camera follows: this entity's translation plus `anchor`.
#[derive(Component)]
pub struct CameraTarget {
    pub anchor: Vec2,
}

/// The follow camera. `focus` is the smoothed world point it frames.
#[derive(Component, Default)]
pub struct FollowCamera {
    pub focus: Vec2,
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct CameraSystems;

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_camera)
            .add_systems(Update, follow.in_set(CameraSystems));
    }
}

fn spawn_camera(mut commands: Commands) {
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
        // Rotation is fixed here; `follow` only translates.
        Transform::default().looking_to(-CAMERA_OFFSET, Vec3::Y),
        FollowCamera::default(),
        cel::camera_bundle(),
    ));
}

/// Glides the focus toward the target (frame-rate independent), snapping on a
/// new target. Reads the target's render-interpolated `Transform`.
fn follow(
    time: Res<Time>,
    target: Single<(Ref<CameraTarget>, &Transform), Without<FollowCamera>>,
    camera: Single<(&mut FollowCamera, &mut Transform)>,
) {
    let (target, target_tf) = *target;
    let goal = target_tf.translation.truncate() + target.anchor;
    let (mut cam, mut tf) = camera.into_inner();
    cam.focus = if target.is_added() {
        goal
    } else {
        let t = 1.0 - (-FOLLOW_STRENGTH * time.delta_secs()).exp();
        cam.focus.lerp(goal, t)
    };
    tf.translation = cam.focus.extend(0.0) + CAMERA_OFFSET + Vec3::Y * FRAME_LIFT;
}
