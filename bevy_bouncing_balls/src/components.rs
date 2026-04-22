//! Composants ECS de la simulation.

use bevy::prelude::*;

#[derive(Component)]
pub struct Ball {
    pub radius: f32,
    pub mass: f32,
}

#[derive(Component)]
pub struct Velocity(pub Vec2);

#[derive(Component)]
pub struct HudText;

#[derive(Component)]
pub struct TraceStatsText;
