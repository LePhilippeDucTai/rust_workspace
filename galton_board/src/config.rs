//! Constantes de configuration de la planche de Galton.

pub const WINDOW_WIDTH: f32 = 1280.0;
pub const WINDOW_HEIGHT: f32 = 900.0;
pub const HALF_WIDTH: f32 = WINDOW_WIDTH * 0.5;
pub const HALF_HEIGHT: f32 = WINDOW_HEIGHT * 0.5;

/// Largeur de la planche (alignée sur les bacs).
pub const BOARD_WIDTH: f32 = 800.0;

/// Marges verticales.
pub const TOP_MARGIN: f32 = 90.0;
pub const BIN_ENTRY_MARGIN: f32 = 26.0;
pub const BOTTOM_MARGIN: f32 = 28.0;

/// Rapport pas_vertical / pas_horizontal entre piquets.
pub const PEG_SPACING_RATIO: f32 = 0.62;

/// Paramètres par défaut, modifiables à l'exécution.
pub const DEFAULT_ROWS: usize = 14;
pub const DEFAULT_PARTICLE_RADIUS: f32 = 2.8;
pub const DEFAULT_TARGET_PARTICLES: usize = 1800;
pub const PEG_RADIUS: f32 = 4.0;
pub const DIVIDER_HALF_WIDTH: f32 = 1.5;

/// Bornes pour les réglages clavier.
pub const MIN_ROWS: usize = 4;
pub const MAX_ROWS: usize = 22;
pub const MIN_PARTICLE_RADIUS: f32 = 1.0;
pub const MAX_PARTICLE_RADIUS: f32 = 8.0;
pub const MIN_TARGET_PARTICLES: usize = 100;
pub const MAX_TARGET_PARTICLES: usize = 6000;

/// Cadence de spawn (particules / seconde).
pub const SPAWN_RATE: f32 = 250.0;

/// Physique.
pub const PHYSICS_HZ: f64 = 240.0;
pub const GRAVITY: f32 = 900.0;
pub const PEG_RESTITUTION: f32 = 0.55;
pub const DIVIDER_RESTITUTION: f32 = 0.35;
pub const WALL_RESTITUTION: f32 = 0.30;
pub const FLOOR_RESTITUTION: f32 = 0.20;
pub const PARTICLE_RESTITUTION: f32 = 0.30;
/// Légère traînée par pas de physique (≈ frottement de l'air).
pub const LINEAR_DAMPING_PER_SEC: f32 = 0.18;

/// Petite asymétrie aléatoire pour éviter qu'une particule reste en équilibre
/// instable sur le sommet d'un piquet.
pub const PEG_JITTER: f32 = 0.12;

/// Compactage colonne aléatoire 2D : fraction d'aire occupée. Sert à la
/// normalisation de la courbe gaussienne théorique en hauteur de pile.
pub const PACKING_EFFICIENCY: f32 = 0.78;

/// Sous-pas de physique par tick fixe (améliore la stabilité des piles denses).
pub const PHYSICS_SUBSTEPS: u32 = 2;
