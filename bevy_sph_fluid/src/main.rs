//! Simulation 2D de fluide par SPH (*Smoothed Particle Hydrodynamics*) avec Bevy.
//!
//! Le fluide est un ensemble de particules dont densité, pression et viscosité
//! sont estimées par noyaux de lissage (Müller et al. 2003). Le solveur vit
//! dans `physics::solver` (découplé de l'ECS, testable) ; le reste n'est que
//! rendu et interaction.
//!
//! Contrôles : `G` gravité, `V` viscosité, `Espace` pause, `R` reset,
//! clic gauche/droit pour repousser/attirer le fluide.

mod components;
mod config;
mod hud;
mod input;
mod physics;
mod resources;
mod spawn;

use bevy::prelude::*;

use crate::config::{PHYSICS_HZ, WINDOW_HEIGHT, WINDOW_WIDTH};
use crate::hud::HudPlugin;
use crate::input::InputPlugin;
use crate::physics::PhysicsPlugin;
use crate::resources::SimSettings;
use crate::spawn::SpawnPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Bevy SPH Fluid".into(),
                resolution: (WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32).into(),
                resizable: false,
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::srgb(0.04, 0.05, 0.09)))
        .insert_resource(SimSettings::default())
        .insert_resource(Time::<Fixed>::from_hz(PHYSICS_HZ))
        .add_plugins((SpawnPlugin, PhysicsPlugin, InputPlugin, HudPlugin))
        .run();
}
