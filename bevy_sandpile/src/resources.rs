//! Ressources globales : grille, réglages, statistiques, histogramme.

use bevy::prelude::*;

use crate::config::{DEFAULT_DROPS_PER_FRAME, HIST_BASE};
use crate::sandpile::Sandpile;

/// Source d'alimentation en grains : où les grains sont déposés.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DropMode {
    /// Dépôt à des positions uniformément aléatoires -> criticité auto-organisée
    /// et avalanches en loi de puissance.
    Random,
    /// Dépôt systématique au centre -> croissance de la célèbre figure fractale.
    Center,
}

impl DropMode {
    pub fn label(self) -> &'static str {
        match self {
            DropMode::Random => "aleatoire (SOC)",
            DropMode::Center => "centre (fractale)",
        }
    }
}

/// La grille de sable, enveloppée pour servir de ressource Bevy.
#[derive(Resource)]
pub struct Grid {
    pub pile: Sandpile,
}

/// Réglages modifiables par l'utilisateur.
#[derive(Resource)]
pub struct SimSettings {
    pub paused: bool,
    pub drops_per_frame: u32,
    pub mode: DropMode,
}

impl Default for SimSettings {
    fn default() -> Self {
        Self {
            paused: false,
            drops_per_frame: DEFAULT_DROPS_PER_FRAME,
            mode: DropMode::Random,
        }
    }
}

/// Compteurs globaux de la simulation.
#[derive(Resource, Default)]
pub struct Stats {
    /// Grains ajoutés depuis le dernier reset.
    pub grains_added: u64,
    /// Effondrements cumulés (tous confondus).
    pub total_topplings: u64,
    /// Nombre d'avalanches non triviales enregistrées (taille >= 1).
    pub avalanches: u64,
    /// Dépôts n'ayant déclenché aucun effondrement.
    pub zero_avalanches: u64,
    /// Plus grande avalanche observée.
    pub max_avalanche: u64,
    /// Taille de la dernière avalanche.
    pub last_avalanche: u64,
}

/// Histogramme à classes logarithmiques de la taille des avalanches.
///
/// La classe `k` regroupe les tailles de l'intervalle `[base^k, base^(k+1))`.
/// Le binning logarithmique est indispensable pour visualiser proprement une
/// loi de puissance sur plusieurs décades.
#[derive(Resource)]
pub struct AvalancheHistogram {
    pub base: f64,
    pub bins: Vec<u64>,
}

impl Default for AvalancheHistogram {
    fn default() -> Self {
        Self {
            base: HIST_BASE,
            bins: Vec::new(),
        }
    }
}

impl AvalancheHistogram {
    pub fn clear(&mut self) {
        self.bins.clear();
    }

    pub fn record(&mut self, size: u64) {
        if size == 0 {
            return;
        }
        let k = ((size as f64).ln() / self.base.ln()).floor() as usize;
        if k >= self.bins.len() {
            self.bins.resize(k + 1, 0);
        }
        self.bins[k] += 1;
    }

    /// Points (log10 taille, log10 densité) pour le tracé log-log.
    ///
    /// La densité divise l'effectif de la classe par sa largeur `base^k(base-1)`,
    /// de sorte que les classes larges ne soient pas artificiellement gonflées.
    pub fn log_log_points(&self) -> Vec<(f64, f64)> {
        let mut pts = Vec::new();
        for (k, &count) in self.bins.iter().enumerate() {
            if count == 0 {
                continue;
            }
            let lo = self.base.powi(k as i32);
            let hi = self.base.powi(k as i32 + 1);
            let width = hi - lo;
            let center = (lo * hi).sqrt();
            let density = count as f64 / width;
            pts.push((center.log10(), density.log10()));
        }
        pts
    }

    /// Pente de la droite des moindres carrés sur les points log-log.
    /// L'exposant de la loi de puissance est l'opposé de cette pente : τ ≈ -pente.
    pub fn fit_slope(&self) -> Option<f64> {
        let pts = self.log_log_points();
        if pts.len() < 2 {
            return None;
        }
        let n = pts.len() as f64;
        let sx: f64 = pts.iter().map(|p| p.0).sum();
        let sy: f64 = pts.iter().map(|p| p.1).sum();
        let sxx: f64 = pts.iter().map(|p| p.0 * p.0).sum();
        let sxy: f64 = pts.iter().map(|p| p.0 * p.1).sum();
        let denom = n * sxx - sx * sx;
        if denom.abs() < 1e-9 {
            return None;
        }
        Some((n * sxy - sx * sy) / denom)
    }
}

/// Handle de la texture représentant la grille.
#[derive(Resource)]
pub struct GridImage(pub Handle<Image>);
