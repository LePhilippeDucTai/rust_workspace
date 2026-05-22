/// Constante gravitationnelle de simulation (G=1000 → L=10 pour conditions initiales canoniques).
pub const G: f32 = 1000.0;

/// Paramètre de softening : évite la singularité quand deux corps se rapprochent trop.
pub const SOFTENING: f32 = 1.5;

/// Nombre max de points par trail.
pub const TRAIL_MAX: usize = 600;

/// Rayon des sphères (unités de simulation).
pub const BODY_RADIUS: f32 = 0.6;

pub const WINDOW_WIDTH: f32 = 1280.0;
pub const WINDOW_HEIGHT: f32 = 800.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gravitational_constant() {
        assert!(G > 0.0);
    }

    #[test]
    fn test_softening_parameter() {
        assert!(SOFTENING > 0.0);
    }

    #[test]
    fn test_trail_configuration() {
        assert!(TRAIL_MAX > 0);
    }

    #[test]
    fn test_body_radius() {
        assert!(BODY_RADIUS > 0.0);
    }

    #[test]
    fn test_window_dimensions() {
        assert!(WINDOW_WIDTH > 0.0);
        assert!(WINDOW_HEIGHT > 0.0);
        assert!(WINDOW_WIDTH > WINDOW_HEIGHT);
    }
}
