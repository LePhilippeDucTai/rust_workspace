//! Raccourcis clavier. La pause est propagée à Rapier via la `RapierConfiguration`
//! du contexte par défaut.

use bevy::prelude::*;
use bevy_rapier2d::prelude::*;

use crate::components::Particle;
use crate::config::{
    MAX_PARTICLE_RADIUS, MAX_ROWS, MAX_TARGET_PARTICLES, MIN_PARTICLE_RADIUS, MIN_ROWS,
    MIN_TARGET_PARTICLES,
};
use crate::resources::{BoardConfig, SimState};

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (handle_input, sync_pause).chain());
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut config: ResMut<BoardConfig>,
    mut state: ResMut<SimState>,
    mut commands: Commands,
    particles: Query<Entity, With<Particle>>,
) {
    if keys.just_pressed(KeyCode::Space) {
        state.paused = !state.paused;
    }
    if keys.just_pressed(KeyCode::KeyG) {
        state.show_gaussian = !state.show_gaussian;
    }
    if keys.just_pressed(KeyCode::KeyH) {
        state.show_histogram_bars = !state.show_histogram_bars;
    }
    if keys.just_pressed(KeyCode::KeyR) {
        for e in &particles {
            commands.entity(e).despawn();
        }
        state.spawned_count = 0;
        state.spawn_accumulator = 0.0;
        for c in state.bin_counts.iter_mut() {
            *c = 0;
        }
    }
    if keys.just_pressed(KeyCode::ArrowRight) && config.rows < MAX_ROWS {
        config.rows += 1;
        state.board_dirty = true;
    }
    if keys.just_pressed(KeyCode::ArrowLeft) && config.rows > MIN_ROWS {
        config.rows -= 1;
        state.board_dirty = true;
    }
    if keys.just_pressed(KeyCode::ArrowUp) && config.particle_radius < MAX_PARTICLE_RADIUS {
        config.particle_radius = (config.particle_radius + 0.5).min(MAX_PARTICLE_RADIUS);
        state.board_dirty = true;
    }
    if keys.just_pressed(KeyCode::ArrowDown) && config.particle_radius > MIN_PARTICLE_RADIUS {
        config.particle_radius = (config.particle_radius - 0.5).max(MIN_PARTICLE_RADIUS);
        state.board_dirty = true;
    }
    let inc = keys.just_pressed(KeyCode::Equal) || keys.just_pressed(KeyCode::NumpadAdd);
    let dec = keys.just_pressed(KeyCode::Minus) || keys.just_pressed(KeyCode::NumpadSubtract);
    if inc && config.target_particles < MAX_TARGET_PARTICLES {
        let step = if config.target_particles >= 1000 { 200 } else { 100 };
        config.target_particles = (config.target_particles + step).min(MAX_TARGET_PARTICLES);
    }
    if dec && config.target_particles > MIN_TARGET_PARTICLES {
        let step = if config.target_particles > 1000 { 200 } else { 100 };
        config.target_particles = config.target_particles.saturating_sub(step).max(MIN_TARGET_PARTICLES);
    }
}

/// Reflète `state.paused` dans la `RapierConfiguration` du contexte par défaut.
fn sync_pause(
    state: Res<SimState>,
    mut rapier_cfg: Query<&mut RapierConfiguration, With<DefaultRapierContext>>,
) {
    if let Ok(mut cfg) = rapier_cfg.single_mut() {
        let target = !state.paused;
        if cfg.physics_pipeline_active != target {
            cfg.physics_pipeline_active = target;
        }
    }
}
