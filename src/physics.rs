use bevy::prelude::*;

use crate::player::Player;

pub const GRAVITY: f32 = 2500.0;
pub const MAX_FALL_SPEED: f32 = 1600.0;

const EPS: f32 = 0.001;

#[derive(Component, Debug, Default, Deref, DerefMut)]
pub struct Velocity(pub Vec2);

#[derive(Component, Debug, Clone, Copy)]
pub struct Collider {
    /// Half-extents of this box collider.
    pub half: Vec2,
    /// One-way platforms only collide with bodies falling from above.
    pub one_way: bool,
}

/// Marks a collider that never moves.
#[derive(Component, Debug)]
pub struct Solid;

#[derive(Debug, Clone, Copy)]
struct Aabb {
    min: Vec2,
    max: Vec2,
}

impl Aabb {
    fn new(center: Vec2, half: Vec2) -> Self {
        Self {
            min: center - half,
            max: center + half,
        }
    }

    fn overlap(&self, o: &Aabb) -> Vec2 {
        let x = (self.max.x.min(o.max.x) - self.min.x.max(o.min.x)).max(0.0);
        let y = (self.max.y.min(o.max.y) - self.min.y.max(o.min.y)).max(0.0);
        Vec2::new(x, y)
    }
}

pub fn apply_gravity(mut q: Query<&mut Velocity>, time: Res<Time<Fixed>>) {
    let dt = time.delta_secs();
    for mut v in &mut q {
        v.y = (v.y - GRAVITY * dt).max(-MAX_FALL_SPEED);
    }
}

/// Moves every body along its velocity, one axis at a time, resolving
/// collisions against static colliders. Axis separation avoids the classic
/// corner-catching problem.
pub fn integrate_and_collide(
    time: Res<Time<Fixed>>,
    mut movers: Query<(&mut Transform, &mut Velocity, &Collider, Option<&mut Player>), Without<Solid>>,
    statics: Query<(&Transform, &Collider), (With<Solid>, Without<Velocity>)>,
) {
    let dt = time.delta_secs();

    for (mut t, mut vel, collider, player) in &mut movers {
        let mut on_ground = false;
        let bottom_before = t.translation.y - collider.half.y;

        // Horizontal sweep.
        t.translation.x += vel.x * dt;
        for (st, sc) in &statics {
            if sc.one_way {
                continue;
            }
            let a = Aabb::new(t.translation.truncate(), collider.half);
            let b = Aabb::new(st.translation.truncate(), sc.half);
            let ov = a.overlap(&b);
            if ov.x > 0.0 && ov.y > 0.0 {
                if t.translation.x < st.translation.x {
                    t.translation.x -= a.max.x - b.min.x;
                } else {
                    t.translation.x += b.max.x - a.min.x;
                }
                vel.x = 0.0;
            }
        }

        // Vertical sweep.
        t.translation.y += vel.y * dt;
        for (st, sc) in &statics {
            let a = Aabb::new(t.translation.truncate(), collider.half);
            let b = Aabb::new(st.translation.truncate(), sc.half);
            let ov = a.overlap(&b);
            if ov.x <= 0.0 || ov.y <= 0.0 {
                continue;
            }

            if sc.one_way {
                if vel.y <= 0.0 && bottom_before >= b.max.y - EPS {
                    t.translation.y = b.max.y + collider.half.y;
                    vel.y = 0.0;
                    on_ground = true;
                }
            } else if vel.y <= 0.0 {
                t.translation.y = b.max.y + collider.half.y;
                vel.y = 0.0;
                on_ground = true;
            } else {
                t.translation.y = b.min.y - collider.half.y;
                vel.y = 0.0;
            }
        }

        if let Some(mut p) = player {
            p.on_ground = on_ground;
        }
    }
}