//! Composants ECS.

use bevy::prelude::*;

/// Un agent (particule) se déplaçant sur la chaîne de Markov.
#[derive(Component)]
pub struct Agent {
    /// État courant (nœud d'arrivée si en transition).
    pub current_state: usize,
    /// État précédent (nœud de départ si en transition).
    pub previous_state: usize,
    /// Progression de la transition en cours [0, 1]. 1.0 = arrivé.
    pub progress: f32,
}

/// Tag : un nœud du graphe.
#[derive(Component)]
pub struct NodeMarker;

/// Tag : étiquette texte d'un nœud (numéro d'état).
#[derive(Component)]
pub struct NodeLabel;

/// Tag : barre d'un histogramme (empirique).
#[derive(Component)]
pub struct HistBar {
    pub state: usize,
}

/// Tag : marqueur de la distribution stationnaire théorique.
#[derive(Component)]
pub struct HistTarget;

/// Tag : texte du HUD principal.
#[derive(Component)]
pub struct HudText;

/// Tag : texte d'information sur la convergence.
#[derive(Component)]
pub struct StatsText;
