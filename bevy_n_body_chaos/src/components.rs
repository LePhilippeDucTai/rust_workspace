use std::collections::VecDeque;
use bevy::prelude::*;

#[derive(Component)]
pub struct Body {
    pub mass: f32,
    pub base_color: Color,
}

#[derive(Component, Default)]
pub struct Trail {
    pub points: VecDeque<Vec3>,
}

#[derive(Component)]
pub struct OrbitCamera {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub target: Vec3,
}

#[derive(Component)]
pub struct HudText;
