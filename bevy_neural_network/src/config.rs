//! Constantes de mise en page et utilitaires partagés.

use bevy::prelude::*;

pub const WINDOW_WIDTH: f32 = 1280.0;
pub const WINDOW_HEIGHT: f32 = 860.0;

/// Diagramme du réseau (à gauche).
pub const NETWORK_RECT: Rect = Rect {
    min: Vec2::new(-630.0, -170.0),
    max: Vec2::new(-30.0, 340.0),
};

/// Espace d'entrée : frontière de décision / courbe de régression (à droite).
pub const DATA_RECT: Rect = Rect {
    min: Vec2::new(40.0, -20.0),
    max: Vec2::new(400.0, 340.0),
};

/// Courbe de loss (bande inférieure).
pub const LOSS_RECT: Rect = Rect {
    min: Vec2::new(-630.0, -410.0),
    max: Vec2::new(400.0, -210.0),
};

/// Résolution de la texture de la frontière de décision.
pub const BOUND_RES: u32 = 160;

/// Nombre de pas SGD effectués par image en mode turbo.
pub const STEPS_PER_TURBO: usize = 150;

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
