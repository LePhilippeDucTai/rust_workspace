//! Composants et marqueurs ECS.

use bevy::prelude::*;

/// Tag : un point d'échantillon accepté (s'accumule pour reconstruire π).
#[derive(Component)]
pub struct SampleDot;

/// Tag : une cellule de la heatmap de la densité cible.
#[derive(Component)]
pub struct HeatCell;

/// Tag : marqueur de l'état courant de la chaîne.
#[derive(Component)]
pub struct CurrentMarker;

/// Tag : marqueur fantôme de la dernière proposition.
#[derive(Component)]
pub struct ProposalGhost;

/// Tag : marqueur de la moyenne empirique courante des échantillons.
#[derive(Component)]
pub struct MeanMarker;

/// Tag : texte d'aide / contrôles du HUD.
#[derive(Component)]
pub struct HudText;

/// Tag : texte des statistiques de convergence.
#[derive(Component)]
pub struct StatsText;
