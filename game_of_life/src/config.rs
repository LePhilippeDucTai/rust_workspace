use bevy::prelude::Color;

pub const GRID_WIDTH: usize = 120;
pub const GRID_HEIGHT: usize = 80;
pub const CELL_SIZE: f32 = 10.0;

pub const WINDOW_WIDTH: f32 = GRID_WIDTH as f32 * CELL_SIZE;
pub const WINDOW_HEIGHT: f32 = GRID_HEIGHT as f32 * CELL_SIZE;

pub const COLOR_ALIVE: Color = Color::srgb(0.22, 0.92, 0.46);
pub const COLOR_DEAD: Color = Color::srgb(0.06, 0.06, 0.10);

pub const SIMULATION_HZ: f64 = 10.0;
pub const MIN_SPEED_HZ: f64 = 1.0;
pub const MAX_SPEED_HZ: f64 = 60.0;
pub const SPEED_FACTOR: f64 = 1.5;

pub const INITIAL_DENSITY: f64 = 0.30;
