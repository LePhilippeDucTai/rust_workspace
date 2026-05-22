//! Constantes de configuration de la simulation.

pub const WINDOW_WIDTH: f32 = 1280.0;
pub const WINDOW_HEIGHT: f32 = 720.0;
pub const HALF_WIDTH: f32 = WINDOW_WIDTH * 0.5;
pub const HALF_HEIGHT: f32 = WINDOW_HEIGHT * 0.5;

pub const INITIAL_BALLS: usize = 100;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_dimensions() {
        assert!(WINDOW_WIDTH > 0.0);
        assert!(WINDOW_HEIGHT > 0.0);
        assert_eq!(HALF_WIDTH, WINDOW_WIDTH * 0.5);
        assert_eq!(HALF_HEIGHT, WINDOW_HEIGHT * 0.5);
    }

    #[test]
    fn test_ball_size_range() {
        assert!(MIN_RADIUS > 0.0);
        assert!(MAX_RADIUS > MIN_RADIUS);
        assert!(INITIAL_BALLS > 0);
    }

    #[test]
    fn test_physics_parameters() {
        assert!(INITIAL_SPEED > 0.0);
        assert!(GRAVITY_ACCEL > 0.0);
        assert!(WALL_RESTITUTION > 0.0);
        assert!(PHYSICS_HZ > 0.0);
    }

    #[test]
    fn test_grid_configuration() {
        assert!(CELL_SIZE > 0.0);
        assert!(INV_CELL > 0.0);
        assert!(GRID_WIDTH > 0);
        assert!(GRID_HEIGHT > 0);
        assert_eq!(GRID_CELLS, GRID_WIDTH * GRID_HEIGHT);
        assert!((CELL_SIZE * INV_CELL - 1.0).abs() < 0.0001);
    }

    #[test]
    fn test_cell_size_relation() {
        assert_eq!(CELL_SIZE, MAX_RADIUS * 2.0);
    }
}
