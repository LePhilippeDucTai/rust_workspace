//! Plugin physique : pilote le solveur SPH et synchronise le rendu.

pub mod solver;

use bevy::prelude::*;

use crate::components::ParticleId;
use crate::config::{DT, GRAVITY, SPEED_FOR_MAX_COLOR, SUBSTEPS, VISCOSITY};
use crate::resources::{not_paused, FluidSim, Interaction, SimSettings};
use solver::ExternalForce;

pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedUpdate, step_fluid.run_if(not_paused))
            // Le rendu suit toujours l'état physique, même en pause.
            .add_systems(Update, render_particles);
    }
}

/// Avance le solveur de `SUBSTEPS` sous-pas par tick (stabilité numérique).
fn step_fluid(
    mut sim: ResMut<FluidSim>,
    settings: Res<SimSettings>,
    interaction: Res<Interaction>,
) {
    let gravity = if settings.gravity_enabled {
        Vec2::new(0.0, GRAVITY)
    } else {
        Vec2::ZERO
    };
    let viscosity = if settings.viscosity_enabled {
        VISCOSITY
    } else {
        0.0
    };
    let external = interaction.active.then_some(ExternalForce {
        center: interaction.center,
        strength: if interaction.repel {
            crate::config::MOUSE_STRENGTH
        } else {
            -crate::config::MOUSE_STRENGTH
        },
    });

    for _ in 0..SUBSTEPS {
        sim.0.step(DT, gravity, viscosity, external);
    }
}

/// Recopie les positions du solveur dans les `Transform` et colore chaque
/// particule selon sa vitesse (bleu lent → cyan/blanc rapide).
fn render_particles(
    sim: Res<FluidSim>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut query: Query<(&ParticleId, &mut Transform, &MeshMaterial2d<ColorMaterial>)>,
) {
    let fluid = &sim.0;
    for (id, mut transform, handle) in &mut query {
        let i = id.0;
        let p = fluid.pos[i];
        transform.translation.x = p.x;
        transform.translation.y = p.y;

        let t = (fluid.vel[i].length() / SPEED_FOR_MAX_COLOR).clamp(0.0, 1.0);
        if let Some(material) = materials.get_mut(&handle.0) {
            material.color = Color::srgb(0.15 + 0.85 * t, 0.45 + 0.55 * t, 1.0);
        }
    }
}
