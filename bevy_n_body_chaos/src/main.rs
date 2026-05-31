//! Simulation N-corps gravitationnelle 3D — Théorie du Chaos.
//!
//! Quatre scénarios : Figure-en-8 (Chenciner-Montgomery), triangle pythagoricien
//! (extrêmement chaotique), triangle de Lagrange (quasi-stable), et aléatoire 3D.
//!
//! Commandes : [1-4] scénario  [R] reset  [Espace] pause  [Souris+drag] orbiter  [Scroll] zoom

mod components;
mod config;
mod hud;
mod physics;
mod resources;
mod scenarios;
mod simulation;
mod visualization;

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;

use bevy::prelude::GlobalAmbientLight;

use crate::config::{WINDOW_HEIGHT, WINDOW_WIDTH};
use crate::hud::HudPlugin;
use crate::physics::PhysicsPlugin;
use crate::resources::{Energy, SimSettings};
use crate::simulation::SimulationPlugin;
use crate::visualization::VisualizationPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "N-Body Chaos — Gravitation 3D".into(),
                resolution: (WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32).into(),
                resizable: false,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(RapierPhysicsPlugin::<NoUserData>::default())
        .insert_resource(ClearColor(Color::srgb(0.02, 0.02, 0.06)))
        .insert_resource(GlobalAmbientLight {
            color: Color::WHITE,
            brightness: 120.0,
            affects_lightmapped_meshes: true,
        })
        .insert_resource(SimSettings::default())
        .insert_resource(Energy::default())
        .add_systems(Startup, disable_rapier_gravity)
        .add_plugins((
            SimulationPlugin,
            PhysicsPlugin,
            VisualizationPlugin,
            HudPlugin,
        ))
        .run();
}

fn disable_rapier_gravity(mut q: Query<&mut RapierConfiguration, With<DefaultRapierContext>>) {
    if let Ok(mut cfg) = q.single_mut() {
        cfg.gravity = Vec3::ZERO;
    }
}
