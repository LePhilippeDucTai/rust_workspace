//! Ressources globales : état de la chaîne, réglages, statistiques.

use std::collections::VecDeque;

use bevy::prelude::*;

use crate::config::{DEFAULT_PROPOSAL_SIGMA, DEFAULT_STEPS_PER_FRAME, START_X, START_Y};

/// État courant de la chaîne de Markov (Metropolis-Hastings).
#[derive(Resource)]
pub struct ChainState {
    pub current: Vec2,
    pub current_density: f32,
    pub last_proposal: Vec2,
    pub last_accepted: bool,
}

impl ChainState {
    pub fn new(density_at: impl Fn(Vec2) -> f32) -> Self {
        let current = Vec2::new(START_X, START_Y);
        Self {
            current,
            current_density: density_at(current),
            last_proposal: current,
            last_accepted: false,
        }
    }
}

/// Réglages modifiables par l'utilisateur.
#[derive(Resource)]
pub struct SimSettings {
    pub paused: bool,
    /// Écart-type de la proposition (marche aléatoire gaussienne symétrique).
    pub proposal_sigma: f32,
    /// Nombre de pas MCMC par frame (vitesse de simulation).
    pub steps_per_frame: u32,
    pub show_heatmap: bool,
    pub show_trace: bool,
}

impl Default for SimSettings {
    fn default() -> Self {
        Self {
            paused: false,
            proposal_sigma: DEFAULT_PROPOSAL_SIGMA,
            steps_per_frame: DEFAULT_STEPS_PER_FRAME,
            show_heatmap: true,
            show_trace: true,
        }
    }
}

/// Compteurs et accumulateurs pour les statistiques de convergence.
#[derive(Resource, Default)]
pub struct Stats {
    pub proposed: u64,
    pub accepted: u64,
    pub recorded: u64,
    /// Somme des échantillons enregistrés (pour la moyenne empirique).
    pub sum: Vec2,
}

impl Stats {
    pub fn acceptance_rate(&self) -> f32 {
        if self.proposed == 0 {
            0.0
        } else {
            self.accepted as f32 / self.proposed as f32
        }
    }

    pub fn empirical_mean(&self) -> Vec2 {
        if self.recorded == 0 {
            Vec2::ZERO
        } else {
            self.sum / self.recorded as f32
        }
    }
}

/// Trace des positions récentes de la chaîne (pour dessiner la marche).
#[derive(Resource, Default)]
pub struct TraceBuffer {
    pub points: VecDeque<Vec2>,
}

/// File des entités-points affichées (recyclage FIFO au-delà de MAX_DOTS).
#[derive(Resource, Default)]
pub struct DotRing {
    pub entities: VecDeque<Entity>,
}

/// Handles partagés pour spawner les points d'échantillon à moindre coût.
#[derive(Resource)]
pub struct DotAssets {
    pub mesh: Handle<Mesh>,
    pub material: Handle<ColorMaterial>,
}
