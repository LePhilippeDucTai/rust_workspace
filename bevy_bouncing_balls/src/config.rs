//! Constantes de configuration de la simulation.

pub const WINDOW_WIDTH: f32 = 1280.0;
pub const WINDOW_HEIGHT: f32 = 720.0;
pub const HALF_WIDTH: f32 = WINDOW_WIDTH * 0.5;
pub const HALF_HEIGHT: f32 = WINDOW_HEIGHT * 0.5;

pub const INITIAL_BALLS: usize = 500;
pub const MIN_RADIUS: f32 = 2.0;
pub const MAX_RADIUS: f32 = 10.0;

pub const INITIAL_SPEED: f32 = 200.0;
pub const GRAVITY_ACCEL: f32 = 800.0;
pub const WALL_RESTITUTION: f32 = 1.0;

pub const PHYSICS_HZ: f64 = 120.0;

// Grille spatiale : cellule = 2 × rayon max. Dimensions calculées pour couvrir
// la fenêtre avec 2 cellules de marge (gère les positions aux bords).
pub const CELL_SIZE: f32 = MAX_RADIUS * 2.0;
pub const INV_CELL: f32 = 1.0 / CELL_SIZE;
pub const GRID_WIDTH: usize = 24; // ⌈1280 / 56⌉ + 2
pub const GRID_HEIGHT: usize = 14; // ⌈720 / 56⌉ + 2
pub const GRID_CELLS: usize = GRID_WIDTH * GRID_HEIGHT;

// En dessous de ce seuil, la résolution des collisions reste séquentielle
// (overhead Rayon > gain pour peu de balles).
pub const PARALLEL_THRESHOLD: usize = 200;
