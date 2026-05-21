//! Planche de Galton avec moteur physique Rapier.
//!
//! Variante de `galton_board` où la physique (gravité, intégration,
//! collisions piquet/mur/particule/particule, CCD) est entièrement déléguée
//! à `bevy_rapier2d`. Le reste (board layout, spawn, HUD, courbe gaussienne,
//! histogramme empirique) est partagé d'esprit avec la version « maison ».

mod board;
mod components;
mod config;
mod gaussian;
mod hud;
mod input;
mod resources;
mod spawn;

use bevy::prelude::*;
use bevy_rapier2d::prelude::*;

use crate::board::BoardPlugin;
use crate::config::{GRAVITY_Y, PIXELS_PER_METER, WINDOW_HEIGHT, WINDOW_WIDTH};
use crate::gaussian::GaussianPlugin;
use crate::hud::HudPlugin;
use crate::input::InputPlugin;
use crate::resources::{BoardConfig, SimState};
use crate::spawn::SpawnPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Planche de Galton (Rapier)".into(),
                resolution: (WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32).into(),
                resizable: false,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(PIXELS_PER_METER))
        .insert_resource(ClearColor(Color::srgb(0.03, 0.04, 0.09)))
        .insert_resource(BoardConfig::default())
        .insert_resource(SimState::default())
        .add_systems(Startup, configure_gravity)
        .add_plugins((BoardPlugin, SpawnPlugin, HudPlugin, InputPlugin, GaussianPlugin))
        .run();
}

/// Surcharge la gravité par défaut de Rapier (≈ -981 px/s² pour 100 px/m)
/// par notre valeur cible. Exécuté en `Startup` *après* l'initialisation du
/// contexte par `RapierPhysicsPlugin` (ce dernier crée son entité en `PreStartup`).
fn configure_gravity(mut q: Query<&mut RapierConfiguration, With<DefaultRapierContext>>) {
    if let Ok(mut cfg) = q.single_mut() {
        cfg.gravity = Vec2::new(0.0, GRAVITY_Y);
    }
}
