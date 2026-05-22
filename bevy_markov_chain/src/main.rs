//! Visualisation interactive d'une chaîne de Markov ergodique.
//!
//! Démontre le théorème de convergence ergodique : pour une chaîne de
//! Markov irréductible et apériodique, la distribution empirique des
//! agents converge vers l'unique distribution stationnaire π satisfaisant
//! π·P = π, indépendamment de l'état initial.

mod components;
mod config;
mod hud;
mod input;
mod markov;
mod resources;
mod simulation;
mod visualization;

use bevy::prelude::*;

use crate::components::Agent;
use crate::config::{INITIAL_AGENTS, WINDOW_HEIGHT, WINDOW_WIDTH};
use crate::hud::HudPlugin;
use crate::input::InputPlugin;
use crate::markov::default_chain;
use crate::resources::{Chain, EmpiricalDistribution, NodePositions, SimSettings};
use crate::simulation::{agent_color_system, spawn_agent, SimulationPlugin};
use crate::visualization::VisualizationPlugin;

fn main() {
    let chain = default_chain();
    let n = chain.num_states();
    let positions = NodePositions::circular(n);

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Bevy Markov Chain - Convergence ergodique".into(),
                resolution: (WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32).into(),
                resizable: false,
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::srgb(0.08, 0.08, 0.12)))
        .insert_resource(Chain(chain))
        .insert_resource(positions)
        .insert_resource(SimSettings::default())
        .insert_resource(EmpiricalDistribution::new(n))
        .add_plugins((SimulationPlugin, VisualizationPlugin, HudPlugin, InputPlugin))
        .add_systems(Startup, setup_scene)
        .add_systems(Update, agent_color_system)
        .run();
}

fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    positions: Res<NodePositions>,
    existing_agents: Query<Entity, With<Agent>>,
) {
    commands.spawn(Camera2d);

    // Spawn initial : tous les agents dans l'état 0, pour bien visualiser
    // la convergence depuis une distribution dégénérée vers π.
    if existing_agents.is_empty() {
        for _ in 0..INITIAL_AGENTS {
            spawn_agent(&mut commands, &mut meshes, &mut materials, &positions, 0);
        }
    }
}
