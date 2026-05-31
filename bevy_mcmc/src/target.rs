//! Densité cible : un mélange de gaussiennes 2D (multimodal).
//!
//! L'échantillonneur Metropolis-Hastings n'a besoin que de la densité
//! (à une constante près) ; ici elle est normalisée pour pouvoir aussi
//! la dessiner et calculer la moyenne théorique exacte.

use bevy::prelude::*;

use crate::config::{DOMAIN_MAX, DOMAIN_MIN};

/// Une composante gaussienne isotrope du mélange.
pub struct GaussComp {
    pub mean: Vec2,
    pub sigma: f32,
    pub weight: f32,
}

/// Densité cible = somme pondérée de gaussiennes.
#[derive(Resource)]
pub struct Target {
    pub comps: Vec<GaussComp>,
    /// Densité maximale (estimée sur grille) pour normaliser la heatmap.
    pub max_density: f32,
    /// Moyenne théorique E[X] = Σ w_i μ_i (les poids somment à 1).
    pub mean: Vec2,
}

impl Target {
    /// Mélange à trois modes bien séparés : illustre le défi du mélange
    /// (mixing) d'une chaîne MCMC face à une cible multimodale.
    pub fn mixture() -> Self {
        let comps = vec![
            GaussComp { mean: Vec2::new(-1.9, -1.3), sigma: 0.70, weight: 0.40 },
            GaussComp { mean: Vec2::new(1.9, 0.6), sigma: 0.55, weight: 0.35 },
            GaussComp { mean: Vec2::new(-0.4, 2.0), sigma: 0.50, weight: 0.25 },
        ];

        let mut mean = Vec2::ZERO;
        for c in &comps {
            mean += c.mean * c.weight;
        }

        let mut target = Self { comps, max_density: 1.0, mean };
        target.max_density = target.estimate_max();
        target
    }

    /// Densité π(p) au point `p`.
    pub fn density(&self, p: Vec2) -> f32 {
        let mut d = 0.0;
        for c in &self.comps {
            let s2 = c.sigma * c.sigma;
            let norm = c.weight / (std::f32::consts::TAU * s2);
            let r2 = (p - c.mean).length_squared();
            d += norm * (-r2 / (2.0 * s2)).exp();
        }
        d
    }

    /// Estime la densité maximale sur une grille fine (pour la normalisation).
    fn estimate_max(&self) -> f32 {
        let n = 160;
        let span = DOMAIN_MAX - DOMAIN_MIN;
        let mut m = 0.0f32;
        for i in 0..=n {
            for j in 0..=n {
                let x = DOMAIN_MIN + span * i as f32 / n as f32;
                let y = DOMAIN_MIN + span * j as f32 / n as f32;
                m = m.max(self.density(Vec2::new(x, y)));
            }
        }
        m
    }
}
