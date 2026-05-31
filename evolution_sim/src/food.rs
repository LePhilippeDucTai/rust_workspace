//! Respawn progressif de la nourriture pour maintenir la pression sélective.
//!
//! On vise `FOOD_TARGET` particules présentes, alimentées par un débit de
//! `FOOD_RESPAWN_RATE` particules / seconde. L'accumulateur permet un débit
//! fractionnaire (ex : 28.7 / s) sans biais d'arrondi.

use bevy::prelude::*;

use crate::components::Food;
use crate::config::{FOOD_RESPAWN_RATE, FOOD_TARGET};
use crate::resources::{RenderAssets, SimState, SimulationSet, not_paused};
use crate::spawn::{random_world_position, spawn_food};

pub struct FoodPlugin;

impl Plugin for FoodPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            respawn_food.in_set(SimulationSet::Food).run_if(not_paused),
        );
    }
}

fn respawn_food(
    mut commands: Commands,
    mut state: ResMut<SimState>,
    assets: Res<RenderAssets>,
    time: Res<Time<Fixed>>,
    food_q: Query<&Food>,
) {
    // Avance le temps simulé global, à utiliser dans le HUD plus tard.
    state.elapsed += time.delta_secs();

    let current = food_q.iter().count();
    if current >= FOOD_TARGET {
        return;
    }

    let missing = FOOD_TARGET - current;
    state.food_respawn_acc += FOOD_RESPAWN_RATE * time.delta_secs();
    let to_spawn = state.food_respawn_acc.floor() as usize;
    if to_spawn == 0 {
        return;
    }
    let to_spawn = to_spawn.min(missing);
    state.food_respawn_acc -= to_spawn as f32;

    let mut rng = rand::thread_rng();
    for _ in 0..to_spawn {
        spawn_food(&mut commands, &assets, random_world_position(&mut rng));
    }
}
