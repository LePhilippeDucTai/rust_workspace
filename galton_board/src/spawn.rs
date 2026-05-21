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
    if state.spawned_count >= config.target_particles { return; }
    state.spawn_accumulator += time.delta_secs() * SPAWN_RATE;
    let to_spawn = (state.spawn_accumulator as usize).min(config.target_particles - state.spawned_count);
    if to_spawn == 0 { return; }
    state.spawn_accumulator -= to_spawn as f32;
    let mesh = meshes.add(Circle::new(config.particle_radius));
    let mut rng = rand::thread_rng();
    for _ in 0..to_spawn {
        spawn_one(&mut commands, &mut materials, &mesh, &config, &dims, &mut rng);
        state.spawned_count += 1;
    }
}

fn spawn_one(commands: &mut Commands, materials: &mut Assets<ColorMaterial>, mesh: &Handle<Mesh>, config: &BoardConfig, dims: &BoardDims, rng: &mut impl Rng) {
    let hue = rng.gen_range(15.0..55.0f32);
    commands.spawn((
        Mesh2d(mesh.clone()),
        MeshMaterial2d(materials.add(Color::hsl(hue, 0.85, 0.62))),
        Transform::from_xyz(0.0, dims.spawn_y, 0.5),
        Particle { radius: config.particle_radius },
        Velocity(Vec2::new(0.0, -15.0)),
    ));
}
