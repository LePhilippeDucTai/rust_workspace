//! Calcul des agrégats de population (moyennes de traits, génération max…).
//!
//! Mis à jour à chaque frame `Update` afin que le HUD reste fluide même
//! quand la simulation est en pause.

use bevy::prelude::*;

use crate::components::{Energy, Food, Genome, Organism};
use crate::resources::PopulationStats;

pub struct StatsPlugin;

impl Plugin for StatsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PopulationStats>()
            .add_systems(Update, recompute_stats);
    }
}

fn recompute_stats(
    mut stats: ResMut<PopulationStats>,
    organisms: Query<(&Genome, &Energy, &Organism)>,
    foods: Query<&Food>,
) {
    let mut count = 0usize;
    let mut sum_speed = 0.0f64;
    let mut sum_size = 0.0f64;
    let mut sum_vision = 0.0f64;
    let mut sum_energy = 0.0f64;
    let mut sum_gen = 0.0f64;
    let mut max_gen = 0u32;

    for (genome, energy, org) in &organisms {
        count += 1;
        sum_speed += genome.speed as f64;
        sum_size += genome.size as f64;
        sum_vision += genome.vision_range as f64;
        sum_energy += energy.0 as f64;
        sum_gen += org.generation as f64;
        if org.generation > max_gen {
            max_gen = org.generation;
        }
    }

    let denom = count.max(1) as f64;
    stats.count = count;
    stats.food_count = foods.iter().count();
    stats.mean_speed = (sum_speed / denom) as f32;
    stats.mean_size = (sum_size / denom) as f32;
    stats.mean_vision = (sum_vision / denom) as f32;
    stats.mean_energy = (sum_energy / denom) as f32;
    stats.mean_generation = (sum_gen / denom) as f32;
    stats.max_generation = max_gen;
}
