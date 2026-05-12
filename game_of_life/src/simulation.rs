use bevy::prelude::*;

use crate::grid::Grid;

#[derive(Resource, Default)]
pub struct SimState {
    pub paused: bool,
    pub generation: u64,
}

pub struct SimulationPlugin;

impl Plugin for SimulationPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(SimState::default())
            .add_systems(FixedUpdate, step);
    }
}

fn step(mut grid: ResMut<Grid>, mut state: ResMut<SimState>) {
    if state.paused {
        return;
    }
    grid.step();
    state.generation += 1;
}
