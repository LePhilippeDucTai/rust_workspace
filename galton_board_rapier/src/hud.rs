//! HUD : état + paramètres + stats sur les bacs.

use bevy::prelude::*;

use crate::components::{HudText, Particle, StatsText};
use crate::resources::{BoardConfig, SimState};

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
        TextFont {
            font_size: 14.0,
            ..default()
        },
        TextColor(Color::srgb(0.92, 0.94, 0.98)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(8.0),
            left: Val::Px(12.0),
            ..default()
        },
        HudText,
    ));
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: 13.0,
            ..default()
        },
        TextColor(Color::srgb(0.60, 1.0, 0.70)),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(8.0),
            left: Val::Px(12.0),
            ..default()
        },
        StatsText,
    ));
}

fn update_hud(
    config: Res<BoardConfig>,
    state: Res<SimState>,
    time: Res<Time>,
    particles: Query<&Particle>,
    mut hud_q: Query<&mut Text, (With<HudText>, Without<StatsText>)>,
    mut stats_q: Query<&mut Text, (With<StatsText>, Without<HudText>)>,
) {
    let fps = 1.0 / time.delta_secs().max(1e-6);
    let alive = particles.iter().count();
    if let Ok(mut text) = hud_q.single_mut() {
        let pause = if state.paused { "PAUSE" } else { "RUN  " };
        **text = format!(
            "Planche de Galton (Rapier)  —  {pause}  |  FPS: {fps:5.0}\n\
             Rangées (← →) : {rows}    Bacs : {bins}    σ_x ≈ {sigma:.1} px\n\
             Rayon particule (↑ ↓) : {radius:.1} px\n\
             Cible (- +) : {target}    Vivantes : {alive}    Spawn : {spawned} / {target}\n\
             [Espace] pause  [R] reset  [G] courbe gaussienne  [H] barres histogramme",
            rows = config.rows,
            bins = config.num_bins(),
            sigma = config.sigma_x(),
            radius = config.particle_radius,
            target = config.target_particles,
            spawned = state.spawned_count,
        );
    }
    if let Ok(mut text) = stats_q.single_mut() {
        let counted: usize = state.bin_counts.iter().sum();
        let (mean, variance) = mean_var(&state.bin_counts, &config);
        let std = variance.sqrt();
        let sigma_theo = config.sigma_x();
        **text = format!(
            "Bacs (comptage) : {counted} particules\n\
             μ empirique : {mean:+6.2} px    σ empirique : {std:5.2} px\n\
             σ théorique : {sigma_theo:5.2} px  (modèle binomial : d·√N, d = pas/2)",
        );
    }
}

fn mean_var(counts: &[usize], cfg: &BoardConfig) -> (f32, f32) {
    let total: usize = counts.iter().sum();
    if total == 0 {
        return (0.0, 0.0);
    }
    let n = total as f32;
    let mut mean = 0.0;
    for (i, &c) in counts.iter().enumerate() {
        if c == 0 {
            continue;
        }
        mean += cfg.bin_center_x(i) * c as f32;
    }
    mean /= n;
    let mut var = 0.0;
    for (i, &c) in counts.iter().enumerate() {
        if c == 0 {
            continue;
        }
        let d = cfg.bin_center_x(i) - mean;
        var += d * d * c as f32;
    }
    var /= n;
    (mean, var)
}
