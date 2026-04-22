use bevy::prelude::*;

use crate::components::{Ball, Velocity};

pub fn integrate_positions(
    time: Res<Time<Fixed>>,
    mut query: Query<(&mut Transform, &Velocity), With<Ball>>,
) {
    let dt = time.delta_secs();
    for (mut t, v) in &mut query {
        t.translation.x += v.0.x * dt;
        t.translation.y += v.0.y * dt;
    }
}
