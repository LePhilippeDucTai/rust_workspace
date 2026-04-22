use bevy::prelude::*;

use crate::components::{Ball, Velocity};
use crate::config::{WALL_RESTITUTION, WINDOW_HEIGHT, WINDOW_WIDTH};

pub fn resolve_wall_collisions(mut query: Query<(&mut Transform, &mut Velocity, &Ball)>) {
    let half_w = WINDOW_WIDTH * 0.5;
    let half_h = WINDOW_HEIGHT * 0.5;
    for (mut t, mut v, b) in &mut query {
        if t.translation.x - b.radius < -half_w {
            t.translation.x = -half_w + b.radius;
            if v.0.x < 0.0 {
                v.0.x = -v.0.x * WALL_RESTITUTION;
            }
        } else if t.translation.x + b.radius > half_w {
            t.translation.x = half_w - b.radius;
            if v.0.x > 0.0 {
                v.0.x = -v.0.x * WALL_RESTITUTION;
            }
        }
        if t.translation.y - b.radius < -half_h {
            t.translation.y = -half_h + b.radius;
            if v.0.y < 0.0 {
                v.0.y = -v.0.y * WALL_RESTITUTION;
            }
        } else if t.translation.y + b.radius > half_h {
            t.translation.y = half_h - b.radius;
            if v.0.y > 0.0 {
                v.0.y = -v.0.y * WALL_RESTITUTION;
            }
        }
    }
}
