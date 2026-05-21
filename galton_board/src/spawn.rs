//! Spawn progressif des particules à raison de SPAWN_RATE par seconde.

use bevy::prelude::*;
use rand::Rng;

use crate::components::{Particle, Velocity};
use crate::config::SPAWN_RATE;
use crate::resources::{BoardConfig, BoardDims, SimState, not_paused};

pub struct SpawnPlugin;

impl Plugin for SpawnPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, spawn_particles.run_if(not_paused));
    }
}

fn spawn_particles(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    config: Res<BoardConfig>,
    dims: Res<BoardDims>,
    mut state: ResMut<SimState>,
    time: Res<Time>,
) {
    if state.spawned_count >= config.target_particles {
        return;
    }
    state.spawn_accumulator += time.delta_secs() * SPAWN_RATE;
    let to_spawn = state.spawn_accumulator as usize;
    if to_spawn == 0 {
        return;
    }
    state.spawn_accumulator -= to_spawn as f32;

    let radius = config.particle_radius;
    let mesh = meshes.add(Circle::new(radius));
    // Couleur unique pour toutes les particules : économise les allocs de
    // matériau et garde la palette lisible.
    let mut rng = rand::thread_rng();
    let remaining = config.target_particles - state.spawned_count;
    let count = to_spawn.min(remaining);

    for _ in 0..count {
        let jitter: f32 = rng.gen_range(-2.0..=2.0);
        let vx: f32 = rng.gen_range(-10.0..=10.0);
        let hue = rng.gen_range(15.0..55.0);
        let color = Color::hsl(hue, 0.85, 0.62);
        commands.spawn((
            Mesh2d(mesh.clone()),
            MeshMaterial2d(materials.add(color)),
            Transform::from_xyz(jitter, dims.spawn_y, 0.5),
            Particle { radius },
            Velocity(Vec2::new(vx, -20.0)),
        ));
        state.spawned_count += 1;
    }
}
