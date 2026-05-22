//! Constantes de configuration de la planche de Galton (variante Rapier).

pub const WINDOW_WIDTH: f32 = 1280.0;
pub const WINDOW_HEIGHT: f32 = 900.0;
pub const HALF_HEIGHT: f32 = WINDOW_HEIGHT * 0.5;

pub const BOARD_WIDTH: f32 = 800.0;
pub const TOP_MARGIN: f32 = 90.0;
pub const BIN_ENTRY_MARGIN: f32 = 26.0;
pub const BOTTOM_MARGIN: f32 = 28.0;
pub const PEG_SPACING_RATIO: f32 = 0.62;

pub const DEFAULT_ROWS: usize = 14;
pub const DEFAULT_PARTICLE_RADIUS: f32 = 2.8;
pub const DEFAULT_TARGET_PARTICLES: usize = 1800;
pub const PEG_RADIUS: f32 = 4.0;
pub const DIVIDER_HALF_WIDTH: f32 = 1.5;

pub const MIN_ROWS: usize = 4;
pub const MAX_ROWS: usize = 22;
pub const MIN_PARTICLE_RADIUS: f32 = 1.0;
pub const MAX_PARTICLE_RADIUS: f32 = 8.0;
pub const MIN_TARGET_PARTICLES: usize = 100;
pub const MAX_TARGET_PARTICLES: usize = 6000;

pub const SPAWN_RATE: f32 = 250.0;

/// Gravité passée à Rapier (axe Bevy : +Y vers le haut, donc négatif pour
/// faire tomber les particules).
pub const GRAVITY_Y: f32 = -900.0;

/// Conversion pixels ↔ mètres pour les paramètres internes du solveur Rapier.
pub const PIXELS_PER_METER: f32 = 100.0;

/// Coefficients de matière (élasticité, frottement, traînée) — bornés [0,1].
pub const PARTICLE_RESTITUTION: f32 = 0.35;
pub const PARTICLE_FRICTION: f32 = 0.0;
pub const PARTICLE_LINEAR_DAMPING: f32 = 0.15;
pub const PARTICLE_ANGULAR_DAMPING: f32 = 0.6;

pub const PEG_RESTITUTION: f32 = 0.55;
pub const PEG_FRICTION: f32 = 0.2;

pub const WALL_RESTITUTION: f32 = 0.25;
pub const WALL_FRICTION: f32 = 0.4;

pub const PACKING_EFFICIENCY: f32 = 0.78;
