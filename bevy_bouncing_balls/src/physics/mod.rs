//! Systèmes de physique : gravité, intégration, murs, collisions balle-balle.

mod collisions;
mod gravity;
mod integration;
mod walls;

use bevy::prelude::*;

use crate::resources::not_paused;

pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            (
                gravity::apply_gravity,
                integration::integrate_positions,
                collisions::resolve_ball_collisions,
                walls::resolve_wall_collisions,
            )
                .chain()
                .run_if(not_paused),
        );
    }
}
