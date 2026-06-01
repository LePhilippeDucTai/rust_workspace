//! Avancement de la simulation : dépôt de grains et enregistrement des avalanches.

use bevy::prelude::*;
use rand::Rng;

use crate::resources::{AvalancheHistogram, DropMode, Grid, SimSettings, Stats};

pub struct SimulationPlugin;

impl Plugin for SimulationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, step);
    }
}

/// Dépose `drops_per_frame` grains, stabilise après chacun, et accumule les
/// statistiques de taille d'avalanche.
fn step(
    mut grid: ResMut<Grid>,
    mut stats: ResMut<Stats>,
    mut hist: ResMut<AvalancheHistogram>,
    settings: Res<SimSettings>,
) {
    if settings.paused {
        return;
    }

    let mut rng = rand::thread_rng();
    let n = grid.pile.cells.len();
    let center = grid.pile.center();

    for _ in 0..settings.drops_per_frame {
        let i = match settings.mode {
            DropMode::Random => rng.gen_range(0..n),
            DropMode::Center => center,
        };

        let av = grid.pile.add_grain(i);

        stats.grains_added += 1;
        stats.total_topplings += av.topplings;
        if av.topplings == 0 {
            stats.zero_avalanches += 1;
        } else {
            stats.avalanches += 1;
            stats.last_avalanche = av.topplings;
            stats.max_avalanche = stats.max_avalanche.max(av.topplings);
            hist.record(av.topplings);
        }
    }
}
