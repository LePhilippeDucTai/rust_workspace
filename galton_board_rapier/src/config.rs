//! Constantes de configuration de la planche de Galton (variante Rapier).

pub const WINDOW_WIDTH: f32 = 1280.0;
pub const WINDOW_HEIGHT: f32 = 900.0;
pub const HALF_HEIGHT: f32 = WINDOW_HEIGHT * 0.5;

pub const BOARD_WIDTH: f32 = 800.0;
pub const TOP_MARGIN: f32 = 90.0;
pub const BIN_ENTRY_MARGIN: f32 = 26.0;
pub const BOTTOM_MARGIN: f32 = 28.0;
// Triangles équilatéraux : √3/2 ≈ 0.866 — chaque piquet est centré dans
// un canal en V symétrique, garantissant p=0.5 à chaque nœud.
pub const PEG_SPACING_RATIO: f32 = 0.866;

pub const DEFAULT_ROWS: usize = 14;
pub const DEFAULT_PARTICLE_RADIUS: f32 = 2.8;
pub const DEFAULT_TARGET_PARTICLES: usize = 1800;
pub const PEG_RADIUS: f32 = 7.0;
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
/// Valeurs calibrées sur un vrai Galton board à billes métalliques.
pub const PARTICLE_RESTITUTION: f32 = 0.40;
pub const PARTICLE_FRICTION: f32 = 0.05;
pub const PARTICLE_LINEAR_DAMPING: f32 = 0.04;
pub const PARTICLE_ANGULAR_DAMPING: f32 = 0.30;

pub const PEG_RESTITUTION: f32 = 0.55;
pub const PEG_FRICTION: f32 = 0.10;

pub const WALL_RESTITUTION: f32 = 0.20;
pub const WALL_FRICTION: f32 = 0.25;

pub const PACKING_EFFICIENCY: f32 = 0.78;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_dimensions() {
        assert!(WINDOW_WIDTH > 0.0);
        assert!(WINDOW_HEIGHT > 0.0);
        assert_eq!(HALF_HEIGHT, WINDOW_HEIGHT * 0.5);
    }

    #[test]
    fn test_board_layout() {
        assert!(BOARD_WIDTH > 0.0);
        assert!(TOP_MARGIN > 0.0);
        assert!(BIN_ENTRY_MARGIN > 0.0);
        assert!(BOTTOM_MARGIN > 0.0);
    }

    #[test]
    fn test_peg_spacing_ratio() {
        assert!(PEG_SPACING_RATIO > 0.0);
        assert!(PEG_SPACING_RATIO < 1.0);
    }

    #[test]
    fn test_row_count_bounds() {
        assert!(MIN_ROWS > 0);
        assert!(MAX_ROWS > MIN_ROWS);
        assert!(DEFAULT_ROWS >= MIN_ROWS);
        assert!(DEFAULT_ROWS <= MAX_ROWS);
    }

    #[test]
    fn test_particle_radius_bounds() {
        assert!(MIN_PARTICLE_RADIUS > 0.0);
        assert!(MAX_PARTICLE_RADIUS > MIN_PARTICLE_RADIUS);
        assert!(DEFAULT_PARTICLE_RADIUS >= MIN_PARTICLE_RADIUS);
        assert!(DEFAULT_PARTICLE_RADIUS <= MAX_PARTICLE_RADIUS);
    }

    #[test]
    fn test_particle_target_bounds() {
        assert!(MIN_TARGET_PARTICLES > 0);
        assert!(MAX_TARGET_PARTICLES > MIN_TARGET_PARTICLES);
        assert!(DEFAULT_TARGET_PARTICLES >= MIN_TARGET_PARTICLES);
        assert!(DEFAULT_TARGET_PARTICLES <= MAX_TARGET_PARTICLES);
    }

    #[test]
    fn test_physics_parameters() {
        assert!(SPAWN_RATE > 0.0);
        assert!(GRAVITY_Y < 0.0);
        assert!(PIXELS_PER_METER > 0.0);
    }

    #[test]
    fn test_material_coefficients() {
        assert!(PARTICLE_RESTITUTION >= 0.0 && PARTICLE_RESTITUTION <= 1.0);
        assert!(PARTICLE_FRICTION >= 0.0 && PARTICLE_FRICTION <= 1.0);
        assert!(PARTICLE_LINEAR_DAMPING >= 0.0 && PARTICLE_LINEAR_DAMPING <= 1.0);
        assert!(PARTICLE_ANGULAR_DAMPING >= 0.0 && PARTICLE_ANGULAR_DAMPING <= 1.0);

        assert!(PEG_RESTITUTION >= 0.0 && PEG_RESTITUTION <= 1.0);
        assert!(PEG_FRICTION >= 0.0 && PEG_FRICTION <= 1.0);

        assert!(WALL_RESTITUTION >= 0.0 && WALL_RESTITUTION <= 1.0);
        assert!(WALL_FRICTION >= 0.0 && WALL_FRICTION <= 1.0);
    }

    #[test]
    fn test_packing_efficiency() {
        assert!(PACKING_EFFICIENCY > 0.0);
        assert!(PACKING_EFFICIENCY <= 1.0);
    }

    #[test]
    fn test_peg_and_particle_sizes() {
        assert!(PEG_RADIUS > DEFAULT_PARTICLE_RADIUS);
    }
}
