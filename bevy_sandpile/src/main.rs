//! Visualisation interactive du tas de sable abélien (Bak-Tang-Wiesenfeld).
//!
//! On déverse des grains sur une grille ; chaque cellule qui atteint 4 grains
//! s'effondre en redistribuant 1 grain à chacun de ses 4 voisins, déclenchant
//! parfois des avalanches en chaîne. Le modèle illustre deux idées profondes à
//! la croisée de l'animation et des probabilités :
//!
//!   * **Criticité auto-organisée** (mode aléatoire) : sans régler le moindre
//!     paramètre, le système s'installe sur un état critique où la taille des
//!     avalanches suit une loi de puissance P(s) ∝ s^(-τ). L'histogramme log-log
//!     affiché à droite le montre en direct (une droite = une loi de puissance),
//!     et la densité moyenne converge spontanément vers ~2,1 grains/cellule.
//!
//!   * **Structure fractale** (mode centre) : en déposant tous les grains au
//!     centre, l'état stable révèle une magnifique figure fractale auto-similaire.
//!
//! Contrôles : [Space] pause, [M] mode, [Haut/Bas] grains par frame,
//! [R] reset, [C] vider l'histogramme.

mod components;
mod config;
mod hud;
mod input;
mod resources;
mod sandpile;
mod simulation;
mod visualization;

use bevy::prelude::*;

use crate::config::{GRID_H, GRID_W, WINDOW_HEIGHT, WINDOW_WIDTH};
use crate::hud::HudPlugin;
use crate::input::InputPlugin;
use crate::resources::{AvalancheHistogram, Grid, SimSettings, Stats};
use crate::sandpile::Sandpile;
use crate::simulation::SimulationPlugin;
use crate::visualization::VisualizationPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Bevy Sandpile - tas de sable abelien (SOC)".into(),
                resolution: (WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32).into(),
                resizable: false,
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::srgb(0.03, 0.03, 0.05)))
        .insert_resource(Grid {
            pile: Sandpile::new(GRID_W, GRID_H),
        })
        .insert_resource(SimSettings::default())
        .insert_resource(Stats::default())
        .insert_resource(AvalancheHistogram::default())
        .add_plugins((SimulationPlugin, VisualizationPlugin, HudPlugin, InputPlugin))
        .add_systems(Startup, setup_camera)
        .run();
}

fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}
