//! Gestion des entrées clavier.

use bevy::prelude::*;

use crate::components::Agent;
use crate::config::{INITIAL_AGENTS, MAX_AGENTS};
use crate::resources::{Chain, EmpiricalDistribution, NodePositions, SimSettings};
use crate::simulation::spawn_agent;

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, handle_input);
    }
}

fn handle_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut settings: ResMut<SimSettings>,
    mut empirical: ResMut<EmpiricalDistribution>,
    chain: Res<Chain>,
    positions: Res<NodePositions>,
    agents: Query<Entity, With<Agent>>,
) {
    if keys.just_pressed(KeyCode::Space) {
        settings.paused = !settings.paused;
    }

    if keys.just_pressed(KeyCode::KeyP) {
        settings.show_matrix = !settings.show_matrix;
    }

    if keys.just_pressed(KeyCode::KeyR) {
        // Reset : supprime tous les agents et en respawn INITIAL_AGENTS.
        for e in agents.iter() {
            commands.entity(e).despawn();
        }
        settings.elapsed_steps = 0;
        let n = chain.0.num_states();
        *empirical = EmpiricalDistribution::new(n);
        for _ in 0..INITIAL_AGENTS {
            // Tous démarrent depuis l'état 0 pour bien voir la convergence.
            spawn_agent(&mut commands, &mut meshes, &mut materials, &positions, 0);
        }
    }

    let add_pressed = keys.just_pressed(KeyCode::Equal)
        || keys.just_pressed(KeyCode::NumpadAdd)
        || keys.just_pressed(KeyCode::KeyA);
    let remove_pressed = keys.just_pressed(KeyCode::Minus)
        || keys.just_pressed(KeyCode::NumpadSubtract)
        || keys.just_pressed(KeyCode::KeyD);

    if add_pressed {
        let current = agents.iter().count();
        let to_add = 50.min(MAX_AGENTS.saturating_sub(current));
        for _ in 0..to_add {
            spawn_agent(&mut commands, &mut meshes, &mut materials, &positions, 0);
        }
    }

    if remove_pressed {
        let to_remove = 50;
        for (i, e) in agents.iter().enumerate() {
            if i >= to_remove {
                break;
            }
            commands.entity(e).despawn();
        }
    }

    // Vitesse de simulation.
    if keys.just_pressed(KeyCode::ArrowUp) {
        settings.steps_per_second = (settings.steps_per_second * 1.25).min(20.0);
    }
    if keys.just_pressed(KeyCode::ArrowDown) {
        settings.steps_per_second = (settings.steps_per_second / 1.25).max(0.1);
    }
}
