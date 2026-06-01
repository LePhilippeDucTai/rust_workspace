//! Composants et marqueurs ECS.

use bevy::prelude::*;

/// Tag : le sprite affichant la texture de la grille.
#[derive(Component)]
pub struct GridSprite;

/// Tag : texte d'aide / contrôles du HUD.
#[derive(Component)]
pub struct HudText;

/// Tag : texte des statistiques de criticité.
#[derive(Component)]
pub struct StatsText;
