//! HUD : contrôles et statistiques de convergence.

use bevy::prelude::*;

use crate::components::{HudText, StatsText};
use crate::resources::{SimSettings, Stats};
use crate::target::Target;

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_hud)
            .add_systems(Update, update_hud);
    }
}

fn setup_hud(mut commands: Commands) {
    commands.spawn((
        Text::new(""),
        TextFont { font_size: 14.0, ..default() },
        TextColor(Color::srgb(0.92, 0.92, 0.96)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(8.0),
            left: Val::Px(10.0),
            ..default()
        },
        HudText,
    ));

    commands.spawn((
        Text::new(""),
        TextFont { font_size: 14.0, ..default() },
        TextColor(Color::srgb(0.85, 1.0, 0.9)),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(8.0),
            left: Val::Px(10.0),
            ..default()
        },
        StatsText,
    ));
}

fn update_hud(
    settings: Res<SimSettings>,
    stats: Res<Stats>,
    target: Res<Target>,
    time: Res<Time>,
    mut hud_q: Query<&mut Text, (With<HudText>, Without<StatsText>)>,
    mut stats_q: Query<&mut Text, (With<StatsText>, Without<HudText>)>,
) {
    let fps = 1.0 / time.delta_secs().max(1e-6);
    let on = |b: bool| if b { "ON" } else { "OFF" };

    if let Ok(mut text) = hud_q.single_mut() {
        **text = format!(
            "Echantillonnage MCMC - Metropolis-Hastings (marche aleatoire)\n\
             Cible pi: melange de 3 gaussiennes 2D   FPS: {fps:.0}\n\
             [Space] pause: {}   [Up/Down] vitesse: {} pas/frame\n\
             [Left/Right] sigma proposition: {:.2}   [H] heatmap: {}   [T] trace: {}   [R] reset",
            on(settings.paused),
            settings.steps_per_frame,
            settings.proposal_sigma,
            on(settings.show_heatmap),
            on(settings.show_trace),
        );
    }

    if let Ok(mut text) = stats_q.single_mut() {
        let emp = stats.empirical_mean();
        let true_mean = target.mean;
        let err = (emp - true_mean).length();
        let acc = stats.acceptance_rate();
        **text = format!(
            "Acceptation: {:.1}%  (optimum theorique ~23.4%)   Echantillons enregistres: {}\n\
             Moyenne empirique: ({:.3}, {:.3})   vs  E[X] theorique: ({:.3}, {:.3})\n\
             Erreur ||moyenne_emp - E[X]||: {err:.4}   -> tend vers 0 (loi des grands nombres pour MCMC)\n\
             cyan = etat courant | magenta = moyenne empirique | croix verte = E[X] | croix blanches = modes",
            acc * 100.0,
            stats.recorded,
            emp.x, emp.y,
            true_mean.x, true_mean.y,
        );
    }
}
