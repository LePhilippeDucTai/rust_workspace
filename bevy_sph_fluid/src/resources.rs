//! Ressources globales et conditions d'exécution.

use bevy::prelude::*;

use crate::physics::solver::Fluid;

/// État physique du fluide (source de vérité ; les `Transform` ne sont que
/// des miroirs de rendu, synchronisés chaque frame).
#[derive(Resource)]
pub struct FluidSim(pub Fluid);

/// Positions initiales du bloc de fluide, conservées pour la réinitialisation.
#[derive(Resource)]
pub struct InitialLayout(pub Vec<Vec2>);

/// Réglages basculables depuis le clavier.
#[derive(Resource)]
pub struct SimSettings {
    pub gravity_enabled: bool,
    pub viscosity_enabled: bool,
    pub paused: bool,
}

impl Default for SimSettings {
    fn default() -> Self {
        Self {
            gravity_enabled: true,
            viscosity_enabled: true,
            paused: false,
        }
    }
}

/// Forçage par le curseur, mis à jour par le système d'entrées.
#[derive(Resource, Default)]
pub struct Interaction {
    pub active: bool,
    pub center: Vec2,
    /// `true` ⇒ répulsion, `false` ⇒ attraction.
    pub repel: bool,
}

pub fn not_paused(settings: Res<SimSettings>) -> bool {
    !settings.paused
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_settings() {
        let s = SimSettings::default();
        assert!(s.gravity_enabled);
        assert!(s.viscosity_enabled);
        assert!(!s.paused);
    }

    #[test]
    fn test_paused_flag_toggles() {
        let mut s = SimSettings::default();
        assert!(!s.paused);
        s.paused = true;
        assert!(s.paused);
    }

    #[test]
    fn test_interaction_default_inactive() {
        let i = Interaction::default();
        assert!(!i.active);
    }
}
