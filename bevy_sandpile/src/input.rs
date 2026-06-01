//! Gestion des entrées clavier.

use bevy::prelude::*;

use crate::config::MAX_DROPS_PER_FRAME;
use crate::resources::{AvalancheHistogram, DropMode, Grid, SimSettings, Stats};

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, handle_input);
    }
}

fn handle_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut settings: ResMut<SimSettings>,
    mut grid: ResMut<Grid>,
    mut stats: ResMut<Stats>,
    mut hist: ResMut<AvalancheHistogram>,
) {
    if keys.just_pressed(KeyCode::Space) {
        settings.paused = !settings.paused;
    }

    if keys.just_pressed(KeyCode::KeyM) {
        settings.mode = match settings.mode {
            DropMode::Random => DropMode::Center,
            DropMode::Center => DropMode::Random,
        };
    }

    if keys.just_pressed(KeyCode::ArrowUp) {
        settings.drops_per_frame = (settings.drops_per_frame * 2).min(MAX_DROPS_PER_FRAME);
    }
    if keys.just_pressed(KeyCode::ArrowDown) {
        settings.drops_per_frame = (settings.drops_per_frame / 2).max(1);
    }

    // Vide l'histogramme sans toucher à la grille (pour ré-échantillonner la
    // statistique après un changement de régime).
    if keys.just_pressed(KeyCode::KeyC) {
        hist.clear();
        *stats = Stats::default();
    }

    // Reset complet : grille vidée, compteurs et histogramme remis à zéro.
    if keys.just_pressed(KeyCode::KeyR) {
        grid.pile.clear();
        *stats = Stats::default();
        hist.clear();
    }
}
