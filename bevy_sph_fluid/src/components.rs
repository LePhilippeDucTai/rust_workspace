//! Composants ECS de la simulation.

use bevy::prelude::*;

/// Marqueur d'une particule de fluide.
#[derive(Component)]
pub struct Particle;

/// Indice de la particule dans les tableaux du solveur (`FluidSim`).
/// Fait le lien entre l'entité de rendu et l'état physique.
#[derive(Component)]
pub struct ParticleId(pub usize);

/// Texte du HUD.
#[derive(Component)]
pub struct HudText;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_particle_id_stores_index() {
        let id = ParticleId(42);
        assert_eq!(id.0, 42);
    }
}
