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
