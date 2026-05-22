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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_dimensions() {
        assert!(WINDOW_WIDTH > 0.0);
        assert!(WINDOW_HEIGHT > 0.0);
        assert_eq!(WINDOW_WIDTH, GRID_WIDTH as f32 * CELL_SIZE);
        assert_eq!(WINDOW_HEIGHT, GRID_HEIGHT as f32 * CELL_SIZE);
    }

    #[test]
    fn test_speed_bounds() {
        assert!(MIN_SPEED_HZ > 0.0);
        assert!(MAX_SPEED_HZ > MIN_SPEED_HZ);
        assert!(SPEED_FACTOR > 1.0);
    }

    #[test]
    fn test_initial_density() {
        assert!(INITIAL_DENSITY >= 0.0);
        assert!(INITIAL_DENSITY <= 1.0);
    }

    #[test]
    fn test_color_values() {
        let alive = COLOR_ALIVE;
        let dead = COLOR_DEAD;
        assert_ne!(alive, dead);
    }
}
