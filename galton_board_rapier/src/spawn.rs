//! Spawn progressif des particules. Toutes la physique est ensuite gérée par
//! Rapier (gravité, intégration, collisions).

use bevy::prelude::*;
use bevy_rapier2d::prelude::*;
use rand::Rng;

use crate::components::Particle;
use crate::config::{
    PARTICLE_ANGULAR_DAMPING, PARTICLE_FRICTION, PARTICLE_LINEAR_DAMPING, PARTICLE_RESTITUTION,
    SPAWN_RATE,
};
use crate::resources::{BoardConfig, BoardDims, SimState};

pub struct SpawnPlugin;

impl Plugin for SpawnPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, spawn_particles);
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
    if state.paused || state.spawned_count >= config.target_particles { return; }
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
    let hue = rng.gen_range(0.0..360.0f32);
    let radius = config.particle_radius;
    commands.spawn((
        Mesh2d(mesh.clone()),
        MeshMaterial2d(materials.add(Color::hsl(hue, 0.95, 0.68))),
        Transform::from_xyz(0.0, dims.spawn_y, 0.5),
        Particle,
        RigidBody::Dynamic, Collider::ball(radius),
        Restitution::coefficient(PARTICLE_RESTITUTION), Friction::coefficient(PARTICLE_FRICTION),
        Damping { linear_damping: PARTICLE_LINEAR_DAMPING, angular_damping: PARTICLE_ANGULAR_DAMPING },
        LockedAxes::ROTATION_LOCKED, Velocity::linear(Vec2::new(0.0, -15.0)), Ccd::enabled(),
    ));
}
