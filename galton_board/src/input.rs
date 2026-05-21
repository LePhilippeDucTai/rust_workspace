//! Raccourcis clavier : pause, reset, ajustements paramétriques en direct.

use bevy::prelude::*;

use crate::components::Particle;
use crate::config::{
    MAX_PARTICLE_RADIUS, MAX_ROWS, MAX_TARGET_PARTICLES, MIN_PARTICLE_RADIUS, MIN_ROWS,
    MIN_TARGET_PARTICLES,
};
use crate::resources::{BoardConfig, SimState};

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, handle_input);
    }
}

fn handle_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut config: ResMut<BoardConfig>,
    mut state: ResMut<SimState>,
    mut commands: Commands,
    particles: Query<Entity, With<Particle>>,
) {
    handle_toggles(&keys, &mut state);
    handle_reset(&keys, &mut state, &mut commands, &particles);
    handle_rows(&keys, &mut config, &mut state);
    handle_radius(&keys, &mut config, &mut state);
    handle_target(&keys, &mut config);
}

fn handle_toggles(keys: &ButtonInput<KeyCode>, state: &mut SimState) {
    if keys.just_pressed(KeyCode::Space) { state.paused = !state.paused; }
    if keys.just_pressed(KeyCode::KeyG) { state.show_gaussian = !state.show_gaussian; }
    if keys.just_pressed(KeyCode::KeyH) { state.show_histogram_bars = !state.show_histogram_bars; }
}

fn handle_reset(keys: &ButtonInput<KeyCode>, state: &mut SimState, commands: &mut Commands, particles: &Query<Entity, With<Particle>>) {
    if !keys.just_pressed(KeyCode::KeyR) { return; }
    for e in particles { commands.entity(e).despawn(); }
    state.spawned_count = 0;
    state.spawn_accumulator = 0.0;
    state.bin_counts.iter_mut().for_each(|c| *c = 0);
}

fn handle_rows(keys: &ButtonInput<KeyCode>, config: &mut BoardConfig, state: &mut SimState) {
    if keys.just_pressed(KeyCode::ArrowRight) && config.rows < MAX_ROWS {
        config.rows += 1; state.board_dirty = true;
    }
    if keys.just_pressed(KeyCode::ArrowLeft) && config.rows > MIN_ROWS {
        config.rows -= 1; state.board_dirty = true;
    }
}

fn handle_radius(keys: &ButtonInput<KeyCode>, config: &mut BoardConfig, state: &mut SimState) {
    if keys.just_pressed(KeyCode::ArrowUp) && config.particle_radius < MAX_PARTICLE_RADIUS {
        config.particle_radius = (config.particle_radius + 0.5).min(MAX_PARTICLE_RADIUS);
        state.board_dirty = true;
    }
    if keys.just_pressed(KeyCode::ArrowDown) && config.particle_radius > MIN_PARTICLE_RADIUS {
        config.particle_radius = (config.particle_radius - 0.5).max(MIN_PARTICLE_RADIUS);
        state.board_dirty = true;
    }
}

fn handle_target(keys: &ButtonInput<KeyCode>, config: &mut BoardConfig) {
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
