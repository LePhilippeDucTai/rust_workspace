//! Constantes de configuration de la visualisation.

pub const WINDOW_WIDTH: f32 = 1280.0;
pub const WINDOW_HEIGHT: f32 = 800.0;

/// Rayon du cercle d'agencement des nœuds du graphe.
pub const GRAPH_RADIUS: f32 = 230.0;
/// Centre du graphe (décalé à gauche pour laisser place à l'histogramme).
pub const GRAPH_CENTER_X: f32 = -260.0;
pub const GRAPH_CENTER_Y: f32 = 20.0;

/// Rayon visuel d'un nœud.
pub const NODE_RADIUS: f32 = 30.0;
/// Rayon visuel d'un agent (particule).
pub const AGENT_RADIUS: f32 = 4.0;

/// Nombre initial d'agents simulés.
pub const INITIAL_AGENTS: usize = 200;
/// Nombre max d'agents.
pub const MAX_AGENTS: usize = 5000;

/// Centre de l'histogramme.
pub const HIST_CENTER_X: f32 = 360.0;
pub const HIST_BOTTOM_Y: f32 = -200.0;
pub const HIST_WIDTH: f32 = 420.0;
pub const HIST_HEIGHT: f32 = 280.0;

/// Palette de 6 couleurs distinctes pour les états.
pub const STATE_COLORS: [(f32, f32, f32); 6] = [
    (0.92, 0.30, 0.30), // rouge
    (0.30, 0.55, 0.95), // bleu
    (0.35, 0.85, 0.45), // vert
    (0.95, 0.75, 0.20), // jaune
    (0.75, 0.40, 0.90), // violet
    (0.95, 0.55, 0.25), // orange
];
