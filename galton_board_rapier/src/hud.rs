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
    spawn_hud_text(&mut commands);
    spawn_stats_text(&mut commands);
}

fn spawn_hud_text(commands: &mut Commands) {
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
}

fn spawn_stats_text(commands: &mut Commands) {
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
        **text = hud_string(&config, &state, fps, alive);
    }
    if let Ok(mut text) = stats_q.single_mut() {
        **text = stats_string(&state, &config);
    }
}

fn hud_string(config: &BoardConfig, state: &SimState, fps: f32, alive: usize) -> String {
    let pause = if state.paused { "PAUSE" } else { "RUN  " };
    format!(
        "Planche de Galton (Rapier) -- {pause}  |  FPS: {fps:5.0}\n\
         Rangees (<- ->) : {rows}    Bacs : {bins}    sigma_x ~ {sigma:.1} px\n\
         Rayon particule (^ v) : {radius:.1} px\n\
         Cible (- +) : {target}    Vivantes : {alive}    Spawn : {spawned} / {target}\n\
         [Espace] pause  [R] reset  [G] courbe gaussienne  [H] barres histogramme\n\
         [scroll] zoom   [ [ / ] ] zoom clavier",
        rows = config.rows,
        bins = config.num_bins(),
        sigma = config.sigma_x(),
        radius = config.particle_radius,
        target = config.target_particles,
        spawned = state.spawned_count,
    )
}

fn stats_string(state: &SimState, config: &BoardConfig) -> String {
    let counted: usize = state.bin_counts.iter().sum();
    let (mean, var) = mean_var(&state.bin_counts, config);
    format!(
        "Bacs (comptage) : {counted} particules\n\
         mu empirique : {mean:+6.2} px    sigma empirique : {std:5.2} px\n\
         sigma theorique : {sigma:.2} px  (binomial : d*sqrt(N), d = pas/2)",
        std = var.sqrt(),
        sigma = config.sigma_x(),
    )
}

fn mean_var(counts: &[usize], cfg: &BoardConfig) -> (f32, f32) {
    let total: usize = counts.iter().sum();
    if total == 0 {
        return (0.0, 0.0);
    }
    let n = total as f32;
    let mean = counts
        .iter()
        .enumerate()
        .map(|(i, &c)| cfg.bin_center_x(i) * c as f32)
        .sum::<f32>()
        / n;
    let var = counts
        .iter()
        .enumerate()
        .map(|(i, &c)| {
            let d = cfg.bin_center_x(i) - mean;
            d * d * c as f32
        })
        .sum::<f32>()
        / n;
    (mean, var)
}
