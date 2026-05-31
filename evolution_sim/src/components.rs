//! Composants ECS de la simulation d'évolution.
//!
//! Un organisme est une entité Bevy composée de :
//! - `Organism` : tag + identifiant de génération,
//! - `Genome`   : traits hérités (vitesse, taille, vision),
//! - `Energy`   : réserve métabolique,
//! - `Velocity` : direction et norme courantes.
//!
//! La séparation `Genome` / `Energy` permet aux systèmes de borrow chacun
//! indépendamment (utile pour la reproduction qui lit le génome parent et
//! écrit l'énergie en même temps).

use bevy::prelude::*;

/// Tag : l'entité est un organisme vivant.
#[derive(Component, Debug, Clone, Copy)]
pub struct Organism {
    /// Profondeur dans l'arbre généalogique (les fondateurs = 0).
    pub generation: u32,
    /// Temps écoulé depuis la dernière reproduction (cooldown).
    pub time_since_reproduction: f32,
}

impl Organism {
    pub fn founder() -> Self {
        Self {
            generation: 0,
            time_since_reproduction: 0.0,
        }
    }

    pub fn child_of(parent: &Organism) -> Self {
        Self {
            generation: parent.generation + 1,
            time_since_reproduction: 0.0,
        }
    }
}

/// Traits génétiques. Tous bornés dans [min, max] par construction.
#[derive(Component, Debug, Clone, Copy)]
pub struct Genome {
    pub speed: f32,
    pub size: f32,
    pub vision_range: f32,
}

/// Énergie courante de l'organisme. Mort si elle atteint zéro.
#[derive(Component, Debug, Clone, Copy)]
pub struct Energy(pub f32);

/// Vecteur vitesse 2D (pixels/seconde).
#[derive(Component, Debug, Clone, Copy)]
pub struct Velocity(pub Vec2);

/// Tag : particule de nourriture statique.
#[derive(Component, Debug, Clone, Copy)]
pub struct Food;

// ───────────────────────────── UI tags ───────────────────────────────────────

#[derive(Component)]
pub struct HudText;

#[derive(Component)]
pub struct StatsText;
