//! Constantes de configuration : fenêtre, grille, palette, panneau statistique.

pub const WINDOW_WIDTH: f32 = 1280.0;
pub const WINDOW_HEIGHT: f32 = 800.0;

/// Côté de la grille (impair -> il existe une vraie cellule centrale, utile
/// pour la symétrie du mode « pile centrale »).
pub const GRID_W: usize = 181;
pub const GRID_H: usize = 181;

/// Taille d'affichage d'une cellule, en pixels.
pub const CELL_PX: f32 = 3.8;

/// Centre de la grille à l'écran (décalée à gauche pour laisser place au HUD).
pub const GRID_CX: f32 = -260.0;
pub const GRID_CY: f32 = 0.0;

/// Nombre de grains déposés par frame au démarrage.
pub const DEFAULT_DROPS_PER_FRAME: u32 = 150;
pub const MAX_DROPS_PER_FRAME: u32 = 4000;

/// Palette indexée par nombre de grains (0..=3), en octets sRGB RGBA.
/// 0 = vide (fond sombre), puis bleu, orange, jaune clair pour 1, 2, 3.
pub const PALETTE: [[u8; 4]; 4] = [
    [14, 12, 30, 255],   // 0 grain
    [46, 102, 158, 255], // 1 grain
    [232, 128, 38, 255], // 2 grains
    [250, 232, 150, 255], // 3 grains
];

/// Base logarithmique des classes de l'histogramme de tailles d'avalanches.
pub const HIST_BASE: f64 = 1.4;

/// Cadre (en pixels écran) du panneau log-log de l'histogramme.
pub const HIST_LEFT: f32 = 150.0;
pub const HIST_RIGHT: f32 = 600.0;
pub const HIST_BOTTOM: f32 = -250.0;
pub const HIST_TOP: f32 = 170.0;
