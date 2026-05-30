//! Constantes de configuration de la simulation de fluide SPH.

pub const WINDOW_WIDTH: f32 = 1100.0;
pub const WINDOW_HEIGHT: f32 = 720.0;

// Conteneur (boîte) du fluide, en coordonnées monde centrées sur l'origine.
pub const BOX_MARGIN: f32 = 40.0;
pub const BOX_LEFT: f32 = -WINDOW_WIDTH * 0.5 + BOX_MARGIN;
pub const BOX_RIGHT: f32 = WINDOW_WIDTH * 0.5 - BOX_MARGIN;
pub const BOX_BOTTOM: f32 = -WINDOW_HEIGHT * 0.5 + BOX_MARGIN;
pub const BOX_TOP: f32 = WINDOW_HEIGHT * 0.5 - BOX_MARGIN;

// ───────────────── Paramètres SPH (Müller et al. 2003) ─────────────────
// Noyaux 2D ; voir `physics::solver`.

/// Rayon de lissage : porte du noyau et taille de cellule de la grille.
pub const H: f32 = 25.0;
/// Masse d'une particule (uniforme).
pub const MASS: f32 = 150.0;
/// Raideur de l'équation d'état `p = k (ρ − ρ₀)`. Très élevée : avec la
/// pression clampée à zéro, le fluide ne résiste qu'à la compression, donc il
/// faut une EOS raide pour qu'il forme une nappe au lieu de s'aplatir en
/// monocouche. Le clamp de vitesse du solveur garantit la stabilité.
pub const GAS_CONST: f32 = 500000.0;
/// Coefficient de viscosité dynamique.
pub const VISCOSITY: f32 = 150.0;
/// Accélération gravitationnelle (px/s²), appliquée vers le bas.
pub const GRAVITY: f32 = -700.0;
/// Restitution aux parois : négatif ⇒ rebond amorti de la vitesse normale.
pub const BOUND_DAMPING: f32 = -0.4;

// ───────────────────────── Intégration ─────────────────────────
// Plusieurs sous-pas par tick `FixedUpdate` : le pas SPH doit rester petit
// (raideur du noyau spiky) pour éviter l'explosion numérique.
pub const DT: f32 = 0.0008;
pub const SUBSTEPS: u32 = 10;
pub const PHYSICS_HZ: f64 = 60.0;

// ─────────────────── Disposition initiale du fluide ───────────────────
/// Espacement du réseau initial (≈ H/2 ⇒ chaque particule a des voisins).
pub const SPACING: f32 = H * 0.5;
pub const INIT_COLUMNS: usize = 56;
pub const INIT_ROWS: usize = 30;
/// Rayon de rendu d'une particule (sprite circulaire).
pub const PARTICLE_RADIUS: f32 = SPACING * 0.6;

// ───────────────────────── Interaction souris ─────────────────────────
/// Rayon d'influence du curseur.
pub const MOUSE_RADIUS: f32 = 130.0;
/// Intensité de l'accélération radiale appliquée par le curseur.
pub const MOUSE_STRENGTH: f32 = 14000.0;

// ───────────────────────── Coloration ─────────────────────────
/// Vitesse (px/s) au-delà de laquelle la couleur sature (bleu → blanc).
pub const SPEED_FOR_MAX_COLOR: f32 = 700.0;

#[cfg(test)]
#[allow(clippy::assertions_on_constants)]
mod tests {
    use super::*;

    #[test]
    fn test_box_is_centered_and_non_empty() {
        assert!(BOX_RIGHT > BOX_LEFT);
        assert!(BOX_TOP > BOX_BOTTOM);
        assert_eq!(BOX_LEFT, -BOX_RIGHT);
        assert_eq!(BOX_BOTTOM, -BOX_TOP);
    }

    #[test]
    fn test_sph_parameters_positive() {
        assert!(H > 0.0);
        assert!(MASS > 0.0);
        assert!(GAS_CONST > 0.0);
        assert!(VISCOSITY >= 0.0);
        assert!(DT > 0.0);
        assert!(SUBSTEPS > 0);
    }

    #[test]
    fn test_initial_block_fits_in_box() {
        let width = (INIT_COLUMNS as f32 - 1.0) * SPACING;
        let height = (INIT_ROWS as f32 - 1.0) * SPACING;
        assert!(width < BOX_RIGHT - BOX_LEFT);
        assert!(height < BOX_TOP - BOX_BOTTOM);
    }
}
