//! Simulation d'évolution darwinienne en Bevy.
//!
//! Population d'organismes ECS qui se déplacent, mangent, se reproduisent et
//! meurent. La sélection naturelle émerge des coûts énergétiques :
//! aller vite consomme ∝ v², être gros pèse sur le métabolisme, voir loin
//! coûte aussi un peu. Les traits sont hérités avec mutation gaussienne.
//!
//! Architecture en plugins :
//!   - `SpawnPlugin`   : caméra, fondateurs, nourriture initiale.
//!   - `FoodPlugin`    : respawn progressif pour maintenir la pression.
//!   - `BehaviorPlugin`: décision + mouvement + alimentation + collisions monde.
//!   - `LifecyclePlugin`: mort + reproduction asexuée avec mutation.
//!   - `StatsPlugin`   : agrégats live (moyennes, génération…).
//!   - `HudPlugin`     : panneau texte.
//!   - `InputPlugin`   : raccourcis clavier.
//!   - `OverlayPlugin` : gizmos optionnels (portée de vision).
//!
//! Tout le tuning vit dans `config.rs`.

mod behavior;
mod components;
mod config;
mod food;
mod genetics;
mod hud;
mod input;
mod lifecycle;
mod overlay;
mod resources;
mod spawn;
mod stats;

use bevy::prelude::*;

use crate::behavior::BehaviorPlugin;
use crate::config::{SIM_HZ, WINDOW_HEIGHT, WINDOW_WIDTH};
use crate::food::FoodPlugin;
use crate::hud::HudPlugin;
use crate::input::InputPlugin;
use crate::lifecycle::LifecyclePlugin;
use crate::overlay::OverlayPlugin;
use crate::resources::{SimState, SimulationSet};
use crate::spawn::SpawnPlugin;
use crate::stats::StatsPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Évolution darwinienne — Bevy".into(),
                resolution: (WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32).into(),
                resizable: false,
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::srgb(0.05, 0.07, 0.10)))
        .insert_resource(Time::<Fixed>::from_hz(SIM_HZ))
        .init_resource::<SimState>()
        // Ordre global de la simulation : Food → Behavior → Lifecycle.
        .configure_sets(
            FixedUpdate,
            (
                SimulationSet::Food,
                SimulationSet::Behavior,
                SimulationSet::Lifecycle,
            )
                .chain(),
        )
        .add_plugins((
            SpawnPlugin,
            FoodPlugin,
            BehaviorPlugin,
            LifecyclePlugin,
            StatsPlugin,
            HudPlugin,
            InputPlugin,
            OverlayPlugin,
        ))
        .run();
}
