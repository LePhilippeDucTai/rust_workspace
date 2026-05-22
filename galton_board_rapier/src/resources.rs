//! Ressources : config ajustable + géométrie dérivée + état de simulation.

use bevy::prelude::*;

use crate::config::*;

#[derive(Resource, Clone)]
pub struct BoardConfig {
    pub rows: usize,
    pub particle_radius: f32,
    pub target_particles: usize,
}

impl Default for BoardConfig {
    fn default() -> Self {
        Self {
            rows: DEFAULT_ROWS,
            particle_radius: DEFAULT_PARTICLE_RADIUS,
            target_particles: DEFAULT_TARGET_PARTICLES,
        }
    }
}

impl BoardConfig {
    pub fn peg_spacing_x(&self) -> f32 {
        BOARD_WIDTH / (self.rows + 1) as f32
    }
    pub fn peg_spacing_y(&self) -> f32 {
        self.peg_spacing_x() * PEG_SPACING_RATIO
    }
    pub fn num_bins(&self) -> usize {
        self.rows + 1
    }
    pub fn bin_center_x(&self, i: usize) -> f32 {
        (i as f32 - self.rows as f32 * 0.5) * self.peg_spacing_x()
    }
    pub fn sigma_x(&self) -> f32 {
        let d = self.peg_spacing_x() * 0.5;
        d * (self.rows as f32).sqrt()
    }
    pub fn peg_x(&self, row: usize, col: usize) -> f32 {
        (col as f32 - row as f32 * 0.5) * self.peg_spacing_x()
    }
}

#[derive(Resource, Default, Clone, Copy)]
pub struct BoardDims {
    pub peg_top_y: f32,
    pub bin_top_y: f32,
    pub bin_bottom_y: f32,
    pub left_wall_x: f32,
    pub right_wall_x: f32,
    pub spawn_y: f32,
}

impl BoardDims {
    pub fn from_config(cfg: &BoardConfig) -> Self {
        let peg_top_y = HALF_HEIGHT - TOP_MARGIN;
        let peg_bottom_y = peg_top_y - (cfg.rows.saturating_sub(1)) as f32 * cfg.peg_spacing_y();
        let bin_top_y = peg_bottom_y - BIN_ENTRY_MARGIN;
        let bin_bottom_y = -HALF_HEIGHT + BOTTOM_MARGIN;
        let half_board = BOARD_WIDTH * 0.5;
        Self {
            peg_top_y,
            bin_top_y,
            bin_bottom_y,
            left_wall_x: -half_board,
            right_wall_x: half_board,
            // Spawn à l'intérieur de l'entonnoir, près du haut (y_top ≈ HALF_HEIGHT - 10).
            spawn_y: HALF_HEIGHT - 18.0,
        }
    }
}

#[derive(Resource)]
pub struct SimState {
    pub paused: bool,
    pub spawned_count: usize,
    pub spawn_accumulator: f32,
    pub board_dirty: bool,
    pub show_gaussian: bool,
    pub show_histogram_bars: bool,
    pub bin_counts: Vec<usize>,
}

impl Default for SimState {
    fn default() -> Self {
        Self {
            paused: false,
            spawned_count: 0,
            spawn_accumulator: 0.0,
            board_dirty: false,
            show_gaussian: true,
            show_histogram_bars: true,
            bin_counts: Vec::new(),
        }
    }
}
