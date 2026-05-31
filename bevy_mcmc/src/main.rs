//! Visualisation interactive de l'échantillonnage MCMC (Metropolis-Hastings).
//!
//! Démontre comment une simple marche aléatoire, couplée à la règle
//! d'acceptation de Metropolis, produit des échantillons distribués selon
//! une densité cible π arbitraire (ici un mélange de gaussiennes 2D), sans
//! jamais avoir à connaître sa constante de normalisation. On observe :
//!   * le rodage (burn-in) depuis un point de départ médiocre,
//!   * l'accumulation des échantillons qui reconstruit π,
//!   * la moyenne empirique qui converge vers E[X] (LGN pour les chaînes),
//!   * l'effet de l'écart-type de proposition sur le taux d'acceptation.

mod components;
mod config;
mod hud;
mod input;
mod resources;
mod sampler;
mod target;
mod visualization;

use bevy::prelude::*;

use crate::config::{WINDOW_HEIGHT, WINDOW_WIDTH};
use crate::hud::HudPlugin;
use crate::input::InputPlugin;
use crate::resources::{ChainState, DotAssets, DotRing, SimSettings, Stats, TraceBuffer};
use crate::sampler::SamplerPlugin;
use crate::target::Target;
use crate::visualization::VisualizationPlugin;

fn main() {
    let target = Target::mixture();
    let chain = ChainState::new(|p| target.density(p));

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Bevy MCMC - Metropolis-Hastings".into(),
                resolution: (WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32).into(),
                resizable: false,
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::srgb(0.04, 0.03, 0.07)))
        .insert_resource(target)
        .insert_resource(chain)
        .insert_resource(SimSettings::default())
        .insert_resource(Stats::default())
        .insert_resource(TraceBuffer::default())
        .insert_resource(DotRing::default())
        .add_plugins((SamplerPlugin, VisualizationPlugin, HudPlugin, InputPlugin))
        .add_systems(Startup, setup_scene)
        .run();
}

fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.spawn(Camera2d);

    // Handles partagés pour les points d'échantillon : un seul mesh et un
    // seul matériau semi-transparent. Le recouvrement des points rend les
    // zones de forte densité plus lumineuses -> reconstruction visuelle de π.
    let dot_mesh = meshes.add(Circle::new(2.2));
    let dot_material = materials.add(Color::srgba(1.0, 0.95, 0.85, 0.16));
    commands.insert_resource(DotAssets {
        mesh: dot_mesh,
        material: dot_material,
    });
}
