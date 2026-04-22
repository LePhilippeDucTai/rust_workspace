//! Ressources globales et run conditions.

use bevy::prelude::*;

#[derive(Resource)]
pub struct SimSettings {
    pub gravity_enabled: bool,
    pub collisions_enabled: bool,
    pub paused: bool,
}

impl Default for SimSettings {
    fn default() -> Self {
        Self {
            gravity_enabled: false,
            collisions_enabled: true,
            paused: false,
        }
    }
}

pub fn not_paused(settings: Res<SimSettings>) -> bool {
    !settings.paused
}

#[derive(Resource, Default)]
pub struct TraceData {
    pub active: bool,
    pub target: Option<Entity>,
    pub positions: Vec<Vec2>,
}
