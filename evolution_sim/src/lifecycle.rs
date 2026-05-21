//! Cycle de vie : mort par épuisement d'énergie, reproduction asexuée.
//!
//! Ces systèmes tournent dans `FixedUpdate`, ordonnés après le bloc
//! comportement via le label `SimulationSet::Behavior` → `SimulationSet::Lifecycle`.

use bevy::prelude::*;

use crate::components::{Energy, Genome, Organism};
use crate::config::{
    OFFSPRING_ENERGY_RATIO, REPRODUCTION_COOLDOWN, REPRODUCTION_COST, REPRODUCTION_THRESHOLD,
    SOFT_POPULATION_CAP,
};
use crate::genetics::mutate_genome;
use crate::resources::{not_paused, RenderAssets, SimState, SimulationSet};
use crate::spawn::{OrganismSpawn, random_velocity, spawn_organism};

pub struct LifecyclePlugin;

impl Plugin for LifecyclePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            (death_system, reproduction_system)
                .chain()
                .in_set(SimulationSet::Lifecycle)
                .run_if(not_paused),
        );
    }
}

fn death_system(
    mut commands: Commands,
    mut state: ResMut<SimState>,
    q: Query<(Entity, &Energy), With<Organism>>,
) {
    let mut count = 0u64;
    for (entity, energy) in &q {
        if energy.0 <= 0.0 {
            commands.entity(entity).despawn();
            count += 1;
        }
    }
    state.total_deaths += count;
}

fn reproduction_system(
    mut commands: Commands,
    mut materials: ResMut<Assets<ColorMaterial>>,
    assets: Res<RenderAssets>,
    mut state: ResMut<SimState>,
    mut parents_q: Query<(&Transform, &Genome, &mut Energy, &mut Organism), With<Organism>>,
) {
    let population: usize = parents_q.iter().len();
    if population >= SOFT_POPULATION_CAP {
        return;
    }
    let mut budget = SOFT_POPULATION_CAP - population;

    let mut rng = rand::thread_rng();
    let mut births = 0u64;

    for (tf, genome, mut energy, mut organism) in &mut parents_q {
        if budget == 0 {
            break;
        }
        if energy.0 < REPRODUCTION_THRESHOLD {
            continue;
        }
        if organism.time_since_reproduction < REPRODUCTION_COOLDOWN {
            continue;
        }

        energy.0 -= REPRODUCTION_COST;
        organism.time_since_reproduction = 0.0;

        let child_genome = mutate_genome(&mut rng, genome);
        let child_energy = REPRODUCTION_COST * OFFSPRING_ENERGY_RATIO;
        let offset = random_velocity(&mut rng, genome.size + 2.0);
        let child_pos = tf.translation.truncate() + offset;
        let child_velocity = random_velocity(&mut rng, child_genome.speed * 0.5);

        spawn_organism(
            &mut commands,
            &mut materials,
            &assets,
            OrganismSpawn {
                position: child_pos,
                genome: child_genome,
                organism: Organism::child_of(&organism),
                energy: child_energy,
                velocity: child_velocity,
            },
        );

        births += 1;
        budget -= 1;
    }

    state.total_births += births;
}
