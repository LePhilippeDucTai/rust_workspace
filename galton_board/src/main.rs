//! Planche de Galton avec physique réaliste (Bevy 0.18).
//!
//! - Nombre de rangées, rayon des particules, nombre cible : configurables au
//!   clavier (← →, ↑ ↓, - +).
//! - Particules pilotées par gravité, collisions élastiques avec piquets,
//!   cloisons, murs et particule-particule (grille spatiale).
//! - La courbe de densité gaussienne théorique est tracée par-dessus les
//!   piles, paramétrée par les dimensions de la planche
//!   (σ = pas_piquets/2 · √rangées).

mod board;
mod components;
mod config;
mod gaussian;
mod hud;
mod input;
mod physics;
mod resources;
mod spawn;

use bevy::prelude::*;

use crate::board::BoardPlugin;
use crate::config::{PHYSICS_HZ, WINDOW_HEIGHT, WINDOW_WIDTH};
use crate::gaussian::GaussianPlugin;
use crate::hud::HudPlugin;
use crate::input::InputPlugin;
use crate::physics::PhysicsPlugin;
use crate::resources::{BoardConfig, SimState};
use crate::spawn::SpawnPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Planche de Galton".into(),
                resolution: (WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32).into(),
                resizable: false,
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::srgb(0.06, 0.07, 0.12)))
        .insert_resource(BoardConfig::default())
        .insert_resource(SimState::default())
        .insert_resource(Time::<Fixed>::from_hz(PHYSICS_HZ))
        .add_plugins((
            BoardPlugin,
            SpawnPlugin,
            PhysicsPlugin,
            HudPlugin,
            InputPlugin,
            GaussianPlugin,
        ))
        .run();
}
