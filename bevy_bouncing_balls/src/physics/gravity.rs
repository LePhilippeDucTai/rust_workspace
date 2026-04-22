use bevy::prelude::*;

use crate::components::{Ball, Velocity};
use crate::config::GRAVITY_ACCEL;
use crate::resources::SimSettings;

pub fn apply_gravity(
    settings: Res<SimSettings>,
    time: Res<Time<Fixed>>,
    mut query: Query<&mut Velocity, With<Ball>>,
) {
    if !settings.gravity_enabled {
        return;
    }
    let dt = time.delta_secs();
    for mut v in &mut query {
        v.0.y -= GRAVITY_ACCEL * dt;
    }
}
