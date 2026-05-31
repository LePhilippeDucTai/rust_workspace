//! Constantes de configuration et utilitaires de coordonnées.

use bevy::math::Vec2;

pub const WINDOW_WIDTH: f32 = 1280.0;
pub const WINDOW_HEIGHT: f32 = 800.0;

/// Domaine de la densité cible (en unités de la distribution).
pub const DOMAIN_MIN: f32 = -4.5;
pub const DOMAIN_MAX: f32 = 4.5;

/// Demi-taille en pixels de la zone de tracé (carrée).
pub const PLOT_HALF: f32 = 330.0;
/// Centre de la zone de tracé à l'écran (décalé à gauche pour le HUD).
pub const PLOT_CX: f32 = -170.0;
pub const PLOT_CY: f32 = 0.0;

/// Résolution de la heatmap de la cible (cellules par côté).
pub const HEAT_RES: usize = 64;

/// Nombre maximal de points d'échantillons affichés (FIFO).
pub const MAX_DOTS: usize = 6000;

/// Écart-type initial de la proposition (marche aléatoire gaussienne).
pub const DEFAULT_PROPOSAL_SIGMA: f32 = 0.6;
/// Nombre de pas MCMC effectués par frame au démarrage.
pub const DEFAULT_STEPS_PER_FRAME: u32 = 6;
/// Nombre de pas de rodage (burn-in) avant d'enregistrer les échantillons.
pub const BURN_IN: u64 = 50;
/// Longueur de la trace (segments récents de la marche) affichée.
pub const TRACE_LEN: usize = 45;

/// Point de départ volontairement « mauvais » pour bien voir le burn-in.
pub const START_X: f32 = 3.6;
pub const START_Y: f32 = -3.6;

/// Convertit une coordonnée de l'espace de la distribution vers l'écran.
pub fn target_to_screen(p: Vec2) -> Vec2 {
    let scale = (PLOT_HALF * 2.0) / (DOMAIN_MAX - DOMAIN_MIN);
    let mid = 0.5 * (DOMAIN_MIN + DOMAIN_MAX);
    Vec2::new(PLOT_CX + (p.x - mid) * scale, PLOT_CY + (p.y - mid) * scale)
}
