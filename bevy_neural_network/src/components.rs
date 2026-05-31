//! Composants ECS (marqueurs et données attachées aux entités).

use bevy::prelude::*;

/// Tag : un neurone du diagramme (une entité par neurone).
#[derive(Component)]
pub struct NeuronViz;

/// Position logique d'un neurone dans le réseau (pour relire son activation).
#[derive(Component)]
pub struct NeuronIndex {
    pub layer: usize,
    pub idx: usize,
}

/// Tag : le sprite affichant la frontière de décision en fond de l'espace 2D.
#[derive(Component)]
pub struct BoundarySprite;

/// Tag : texte d'aide / contrôles du HUD.
#[derive(Component)]
pub struct HudText;

/// Tag : texte des statistiques d'entraînement.
#[derive(Component)]
pub struct StatsText;
