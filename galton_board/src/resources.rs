//! Ressources globales et géométrie dérivée.

use bevy::prelude::*;

use crate::config::*;

/// Paramètres ajustables à l'exécution.
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
    /// Pas horizontal entre piquets d'une même rangée. Aligné sur la largeur
    /// des bacs : largeur_planche = (rows+1) * pas.
    pub fn peg_spacing_x(&self) -> f32 {
        BOARD_WIDTH / (self.rows + 1) as f32
    }

    /// Pas vertical entre rangées de piquets.
    pub fn peg_spacing_y(&self) -> f32 {
        self.peg_spacing_x() * PEG_SPACING_RATIO
    }

    pub fn num_bins(&self) -> usize {
        self.rows + 1
    }

    /// Centre horizontal du bac d'index `i` (0..=rows).
    pub fn bin_center_x(&self, i: usize) -> f32 {
        (i as f32 - self.rows as f32 * 0.5) * self.peg_spacing_x()
    }

    /// Écart-type horizontal théorique (en pixels) de la position finale.
    /// Modèle : marche aléatoire ±d à chaque rangée, d = pas/2.
    /// Var = N·d², σ = d·√N.
    pub fn sigma_x(&self) -> f32 {
        let d = self.peg_spacing_x() * 0.5;
        d * (self.rows as f32).sqrt()
    }

    /// Position du `col`-ième piquet (0..=row) de la rangée `row` (0..rows).
    /// Disposition triangulaire : rangée r → r+1 piquets centrés sur 0.
    pub fn peg_x(&self, row: usize, col: usize) -> f32 {
        (col as f32 - row as f32 * 0.5) * self.peg_spacing_x()
    }
}

/// Géométrie verticale calculée à partir de la config et de la fenêtre.
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
            spawn_y: HALF_HEIGHT - TOP_MARGIN * 0.4,
        }
    }
}

/// État dynamique de la simulation.
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

pub fn not_paused(state: Res<SimState>) -> bool {
    !state.paused
}
