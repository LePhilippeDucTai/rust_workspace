//! Ressources globales : assets partagés, état de simulation, statistiques.

use bevy::prelude::*;

/// Assets de rendu réutilisés : un mesh "cercle unité" pour tous les
/// organismes (le rayon est appliqué via `Transform::scale`), un mesh
/// carré pour la nourriture, et un matériau vert partagé.
#[derive(Resource)]
pub struct RenderAssets {
    pub unit_circle: Handle<Mesh>,
    pub food_mesh: Handle<Mesh>,
    pub food_material: Handle<ColorMaterial>,
}

/// Toggles / état de simulation modifiables au clavier.
#[derive(Resource)]
pub struct SimState {
    pub paused: bool,
    pub show_vision: bool,
    pub show_help: bool,
    pub elapsed: f32,
    /// Accumulateur pour le respawn progressif de la nourriture.
    pub food_respawn_acc: f32,
    /// Suit le nombre de morts cumulées (pour stats).
    pub total_deaths: u64,
    pub total_births: u64,
}

impl Default for SimState {
    fn default() -> Self {
        Self {
            paused: false,
            show_vision: false,
            show_help: true,
            elapsed: 0.0,
            food_respawn_acc: 0.0,
            total_deaths: 0,
            total_births: 0,
        }
    }
}

/// Statistiques agrégées sur la population vivante, mises à jour chaque frame.
#[derive(Resource, Default, Clone)]
pub struct PopulationStats {
    pub count: usize,
    pub food_count: usize,
    pub mean_speed: f32,
    pub mean_size: f32,
    pub mean_vision: f32,
    pub mean_energy: f32,
    pub max_generation: u32,
    pub mean_generation: f32,
}

pub fn not_paused(state: Res<SimState>) -> bool {
    !state.paused
}
