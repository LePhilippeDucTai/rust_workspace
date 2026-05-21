//! Composants ECS.

use bevy::prelude::*;

#[derive(Component)]
pub struct Particle {
    pub radius: f32,
}

#[derive(Component)]
pub struct Velocity(pub Vec2);

#[derive(Component)]
pub struct Peg {
    pub radius: f32,
}

/// Cloison verticale entre deux bacs : segment vertical à `x` allant de
/// `y_min` à `y_max`, traité comme un rectangle fin pour la collision.
#[derive(Component, Clone, Copy)]
pub struct Divider {
    pub x: f32,
    pub y_min: f32,
    pub y_max: f32,
}

/// Mur (rectangle plein) — utilisé pour les bordures gauche/droite et le sol.
#[derive(Component, Clone, Copy)]
pub struct Wall {
    pub center: Vec2,
    pub half_extents: Vec2,
    pub restitution: f32,
}

/// Tag : tout ce qui constitue la géométrie de la planche (à reconstruire
/// quand on change le nombre de rangées).
#[derive(Component)]
pub struct BoardEntity;

#[derive(Component)]
pub struct HudText;

#[derive(Component)]
pub struct StatsText;
