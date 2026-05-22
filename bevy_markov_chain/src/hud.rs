//! HUD : texte d'aide, statistiques de convergence.

use bevy::prelude::*;

use crate::components::{Agent, HudText, StatsText};
use crate::resources::{Chain, EmpiricalDistribution, SimSettings};

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
        TextColor(Color::srgb(0.85, 1.0, 0.85)),
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
    time: Res<Time>,
    agents: Query<&Agent>,
    chain: Res<Chain>,
    empirical: Res<EmpiricalDistribution>,
    mut hud_q: Query<&mut Text, (With<HudText>, Without<StatsText>)>,
    mut stats_q: Query<&mut Text, (With<StatsText>, Without<HudText>)>,
) {
    let n_agents = agents.iter().count();
    let fps = 1.0 / time.delta_secs().max(1e-6);
    let on = |b: bool| if b { "ON" } else { "OFF" };

    if let Ok(mut text) = hud_q.single_mut() {
        let matrix_str = if settings.show_matrix {
            format_matrix(&chain)
        } else {
            String::new()
        };
        **text = format!(
            "Chaine de Markov ergodique - convergence vers pi\n\
             Agents: {n_agents}   FPS: {fps:.0}   Transitions: {}\n\
             Vitesse: {:.2} transitions/s/agent   Pause [Space]: {}\n\
             [+/-] agents   [R] reset   [P] matrice: {}   [M] modifier (TODO)\n{matrix_str}",
            settings.elapsed_steps,
            settings.steps_per_second,
            on(settings.paused),
            on(settings.show_matrix),
        );
    }

    if let Ok(mut text) = stats_q.single_mut() {
        // Distance L2 entre la distribution empirique et π.
        let (l2, linf) = convergence_distance(&chain, &empirical);
        **text = format!(
            "Convergence ergodique:  ||p_emp - pi||_2 = {l2:.4}   ||.||_inf = {linf:.4}\n\
             Theoreme: pour une chaine irreductible aperiodique, p_n -> pi unique\n\
             (pi resout pi * P = pi, sum pi_i = 1)"
        );
    }
}

fn convergence_distance(chain: &Chain, empirical: &EmpiricalDistribution) -> (f64, f64) {
    if empirical.total < 1.0 {
        return (f64::NAN, f64::NAN);
    }
    let n = chain.0.num_states();
    let mut l2 = 0.0_f64;
    let mut linf = 0.0_f64;
    for i in 0..n {
        let emp = empirical.counts[i] as f64 / empirical.total as f64;
        let diff = (emp - chain.0.stationary[i]).abs();
        l2 += diff * diff;
        if diff > linf {
            linf = diff;
        }
    }
    (l2.sqrt(), linf)
}

fn format_matrix(chain: &Chain) -> String {
    let n = chain.0.num_states();
    let mut s = String::from("\nMatrice P:\n");
    for i in 0..n {
        s.push_str(&format!("  {i}: "));
        for j in 0..n {
            s.push_str(&format!("{:.2} ", chain.0.transition[[i, j]]));
        }
        s.push('\n');
    }
    s.push_str("Pi: ");
    for i in 0..n {
        s.push_str(&format!("{:.3} ", chain.0.stationary[i]));
    }
    s
}
