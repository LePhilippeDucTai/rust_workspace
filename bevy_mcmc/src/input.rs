//! Gestion des entrées clavier.

use bevy::prelude::*;

use crate::components::SampleDot;
use crate::config::{START_X, START_Y};
use crate::resources::{ChainState, DotRing, SimSettings, Stats, TraceBuffer};
use crate::target::Target;

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, handle_input);
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    target: Res<Target>,
    mut settings: ResMut<SimSettings>,
    mut chain: ResMut<ChainState>,
    mut stats: ResMut<Stats>,
    mut trace: ResMut<TraceBuffer>,
    mut ring: ResMut<DotRing>,
    dots: Query<Entity, With<SampleDot>>,
) {
    if keys.just_pressed(KeyCode::Space) {
        settings.paused = !settings.paused;
    }
    if keys.just_pressed(KeyCode::KeyH) {
        settings.show_heatmap = !settings.show_heatmap;
    }
    if keys.just_pressed(KeyCode::KeyT) {
        settings.show_trace = !settings.show_trace;
    }

    // Vitesse de simulation (pas MCMC par frame).
    if keys.just_pressed(KeyCode::ArrowUp) {
        settings.steps_per_frame = (settings.steps_per_frame + 2).min(400);
    }
    if keys.just_pressed(KeyCode::ArrowDown) {
        settings.steps_per_frame = settings.steps_per_frame.saturating_sub(2).max(1);
    }

    // Écart-type de la proposition.
    if keys.just_pressed(KeyCode::ArrowRight) {
        settings.proposal_sigma = (settings.proposal_sigma * 1.2).min(5.0);
    }
    if keys.just_pressed(KeyCode::ArrowLeft) {
        settings.proposal_sigma = (settings.proposal_sigma / 1.2).max(0.02);
    }

    // Reset complet : on repart du point de départ et on vide tout.
    if keys.just_pressed(KeyCode::KeyR) {
        for e in dots.iter() {
            commands.entity(e).despawn();
        }
        ring.entities.clear();
        trace.points.clear();
        *stats = Stats::default();
        let start = Vec2::new(START_X, START_Y);
        chain.current = start;
        chain.current_density = target.density(start);
        chain.last_proposal = start;
        chain.last_accepted = false;
    }
}
