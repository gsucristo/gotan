use bevy::prelude::*;

/// Marks an entity rendered from a fixed-timestep physics state. `current` is
/// the authoritative physics position at the end of the latest tick; `prev` is
/// where that tick started. The `Transform` is only ever a render artifact —
/// physics reads and writes `current`, never trusting a `Transform` that a
/// previous frame's interpolation may have overwritten.
#[derive(Component, Debug)]
pub struct Interpolated {
    pub prev: Vec3,
    pub current: Vec3,
}

impl Interpolated {
    pub fn new(pos: Vec3) -> Self {
        Self {
            prev: pos,
            current: pos,
        }
    }
}

/// First in the physics tick. Shifts the interpolation window and restores the
/// `Transform` to the authoritative position, undoing the render lerp the last
/// frame wrote into it, so movement integrates from truth rather than from a
/// smoothed value.
pub fn capture_prev(mut q: Query<(&mut Transform, &mut Interpolated)>) {
    for (mut t, mut i) in &mut q {
        i.prev = i.current;
        t.translation = i.current;
    }
}

/// Last in the physics tick, after movement and collision. Records the tick's
/// true end position.
pub fn store_current(mut q: Query<(&Transform, &mut Interpolated)>) {
    for (t, mut i) in &mut q {
        i.current = t.translation;
    }
}

/// Runs once per rendered frame. Places each entity between its previous and
/// current physics states according to how far the fixed clock has advanced.
/// Only `Transform` is touched; `current` stays authoritative.
pub fn update_interpolation(time: Res<Time<Fixed>>, mut q: Query<(&Interpolated, &mut Transform)>) {
    let alpha = time.overstep_fraction().clamp(0.0, 1.0);
    for (i, mut t) in &mut q {
        t.translation = i.prev.lerp(i.current, alpha);
    }
}
