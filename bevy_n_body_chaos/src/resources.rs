use bevy::prelude::*;
use crate::config::G;

#[derive(Resource)]
pub struct SimSettings {
    pub g: f32,
    pub paused: bool,
    pub scenario_idx: usize,
    pub reset_requested: bool,
}

impl Default for SimSettings {
    fn default() -> Self {
        Self { g: G, paused: false, scenario_idx: 0, reset_requested: false }
    }
}

#[derive(Resource, Default)]
pub struct Energy {
    pub ke: f32,
    pub pe: f32,
}
