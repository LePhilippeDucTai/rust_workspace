//! Cœur de l'algorithme : Metropolis-Hastings à marche aléatoire.
//!
//! À chaque pas, on propose x' = x + N(0, σ²I) (proposition symétrique),
//! puis on accepte avec probabilité min(1, π(x') / π(x)). La proposition
//! étant symétrique, le ratio de Hastings vaut 1 : c'est l'algorithme de
//! Metropolis. La chaîne ainsi construite a π pour distribution invariante.

use bevy::prelude::*;
use rand::Rng;

use crate::components::SampleDot;
use crate::config::{BURN_IN, MAX_DOTS, TRACE_LEN, target_to_screen};
use crate::resources::{ChainState, DotAssets, DotRing, SimSettings, Stats, TraceBuffer};
use crate::target::Target;

pub struct SamplerPlugin;

impl Plugin for SamplerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, mcmc_step_system);
    }
}

/// Tire un échantillon de la loi normale centrée réduite (Box-Muller).
fn standard_normal<R: Rng>(rng: &mut R) -> f32 {
    let u1: f32 = rng.r#gen::<f32>().max(1e-7);
    let u2: f32 = rng.r#gen::<f32>();
    (-2.0 * u1.ln()).sqrt() * (std::f32::consts::TAU * u2).cos()
}

#[allow(clippy::too_many_arguments)]
fn mcmc_step_system(
    mut commands: Commands,
    target: Res<Target>,
    settings: Res<SimSettings>,
    mut chain: ResMut<ChainState>,
    mut stats: ResMut<Stats>,
    mut trace: ResMut<TraceBuffer>,
    mut ring: ResMut<DotRing>,
    dot: Res<DotAssets>,
) {
    if settings.paused {
        return;
    }

    let mut rng = rand::thread_rng();

    for _ in 0..settings.steps_per_frame {
        // Proposition : marche aléatoire gaussienne isotrope.
        let proposal = chain.current
            + Vec2::new(
                standard_normal(&mut rng) * settings.proposal_sigma,
                standard_normal(&mut rng) * settings.proposal_sigma,
            );
        let d_prop = target.density(proposal);
        stats.proposed += 1;

        // Règle d'acceptation de Metropolis.
        let accept = if chain.current_density <= 0.0 {
            true
        } else {
            (d_prop / chain.current_density) >= rng.r#gen::<f32>()
        };

        if accept {
            chain.current = proposal;
            chain.current_density = d_prop;
            stats.accepted += 1;
        }
        chain.last_proposal = proposal;
        chain.last_accepted = accept;

        // Après le rodage (burn-in), on enregistre l'état courant comme
        // échantillon de π — qu'on ait accepté ou non (on recompte l'état
        // courant à chaque pas, comme il se doit pour un estimateur MCMC).
        if stats.proposed > BURN_IN {
            stats.recorded += 1;
            stats.sum += chain.current;

            let screen = target_to_screen(chain.current);
            let id = commands
                .spawn((
                    Mesh2d(dot.mesh.clone()),
                    MeshMaterial2d(dot.material.clone()),
                    Transform::from_xyz(screen.x, screen.y, 2.0),
                    SampleDot,
                ))
                .id();
            ring.entities.push_back(id);
            if ring.entities.len() > MAX_DOTS {
                if let Some(old) = ring.entities.pop_front() {
                    commands.entity(old).despawn();
                }
            }

            trace.points.push_back(chain.current);
            while trace.points.len() > TRACE_LEN {
                trace.points.pop_front();
            }
        }
    }
}
