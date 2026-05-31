//! Ressources globales (état de la simulation, chaîne de Markov).

use bevy::prelude::*;

use crate::config::{GRAPH_CENTER_X, GRAPH_CENTER_Y, GRAPH_RADIUS};
use crate::markov::MarkovChain;

#[derive(Resource)]
pub struct Chain(pub MarkovChain);

/// Réglages de la simulation modifiables par l'utilisateur.
#[derive(Resource)]
pub struct SimSettings {
    pub paused: bool,
    pub show_matrix: bool,
    pub elapsed_steps: u64,
    /// Vitesse de simulation : nombre de transitions par seconde par agent.
    pub steps_per_second: f32,
}

impl Default for SimSettings {
    fn default() -> Self {
        Self {
            paused: false,
            show_matrix: false,
            elapsed_steps: 0,
            steps_per_second: 1.6,
        }
    }
}

/// Distribution empirique courante (proportion d'agents par état).
#[derive(Resource, Default)]
pub struct EmpiricalDistribution {
    pub counts: Vec<f32>,
    pub total: f32,
}

impl EmpiricalDistribution {
    pub fn new(n: usize) -> Self {
        Self {
            counts: vec![0.0; n],
            total: 0.0,
        }
    }
}

/// Positions 2D des nœuds (calculées une seule fois).
#[derive(Resource)]
pub struct NodePositions(pub Vec<Vec2>);

impl NodePositions {
    pub fn circular(n: usize) -> Self {
        let mut positions = Vec::with_capacity(n);
        let offset = std::f32::consts::FRAC_PI_2;
        for i in 0..n {
            let theta = offset + (i as f32) * std::f32::consts::TAU / n as f32;
            positions.push(Vec2::new(
                GRAPH_CENTER_X + GRAPH_RADIUS * theta.cos(),
                GRAPH_CENTER_Y + GRAPH_RADIUS * theta.sin(),
            ));
        }
        Self(positions)
    }
}
