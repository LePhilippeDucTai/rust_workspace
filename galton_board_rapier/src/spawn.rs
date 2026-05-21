//! Spawn progressif des particules. Toutes la physique est ensuite gérée par
//! Rapier (gravité, intégration, collisions). On verrouille la rotation pour
//! économiser du compute — visuellement invisible sur des disques pleins.

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
    if state.paused || state.spawned_count >= config.target_particles {
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
            Particle,
            RigidBody::Dynamic,
            Collider::ball(radius),
            Restitution::coefficient(PARTICLE_RESTITUTION),
            Friction::coefficient(PARTICLE_FRICTION),
            Damping {
                linear_damping: PARTICLE_LINEAR_DAMPING,
                angular_damping: PARTICLE_ANGULAR_DAMPING,
            },
            LockedAxes::ROTATION_LOCKED,
            Velocity::linear(Vec2::new(vx, -20.0)),
            Ccd::enabled(),
        ));
        state.spawned_count += 1;
    }
}
