//! Conway's Game of Life — Bevy implementation.

mod config;
mod grid;
mod hud;
mod input;
mod render;
mod simulation;

use bevy::prelude::*;

use config::{
    GRID_HEIGHT, GRID_WIDTH, INITIAL_DENSITY, SIMULATION_HZ, WINDOW_HEIGHT, WINDOW_WIDTH,
};
use grid::Grid;
use hud::HudPlugin;
use input::InputPlugin;
use render::RenderPlugin;
use simulation::SimulationPlugin;

fn main() {
    let mut grid = Grid::new(GRID_WIDTH, GRID_HEIGHT);
    grid.randomize(INITIAL_DENSITY);

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Conway's Game of Life".into(),
                resolution: (WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32).into(),
                resizable: false,
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::srgb(0.04, 0.04, 0.07)))
        .insert_resource(grid)
        .insert_resource(Time::<Fixed>::from_hz(SIMULATION_HZ))
        .add_plugins((SimulationPlugin, RenderPlugin, InputPlugin, HudPlugin))
        .add_systems(Startup, spawn_camera)
        .run();
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}
