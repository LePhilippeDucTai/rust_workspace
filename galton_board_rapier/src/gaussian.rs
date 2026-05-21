//! Comptage par bac + tracé de la densité gaussienne théorique.
//! Identique à la version « maison » : la mesure se fait sur les `Transform`
//! des particules, peu importe qui les a déplacées.

use bevy::prelude::*;
use std::f32::consts::PI;

use crate::components::Particle;
use crate::config::PACKING_EFFICIENCY;
use crate::resources::{BoardConfig, BoardDims, SimState};

pub struct GaussianPlugin;

impl Plugin for GaussianPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (count_bins, draw_gaussian, draw_histogram).chain());
    }
}

fn count_bins(
    config: Res<BoardConfig>,
    dims: Res<BoardDims>,
    mut state: ResMut<SimState>,
    particles: Query<&Transform, With<Particle>>,
) {
    let nb = config.num_bins();
    if state.bin_counts.len() != nb {
        state.bin_counts = vec![0; nb];
    } else {
        for c in state.bin_counts.iter_mut() {
            *c = 0;
        }
    }
    let s = config.peg_spacing_x();
    let half_n = config.rows as f32 * 0.5;
    let bin_top = dims.bin_top_y;
    for tf in &particles {
        if tf.translation.y > bin_top {
            continue;
        }
        let idx_f = tf.translation.x / s + half_n;
        let idx = idx_f.round() as i32;
        if idx >= 0 && (idx as usize) < nb {
            state.bin_counts[idx as usize] += 1;
        }
    }
}

fn draw_gaussian(
    config: Res<BoardConfig>,
    dims: Res<BoardDims>,
    state: Res<SimState>,
    mut gizmos: Gizmos,
) {
    if !state.show_gaussian {
        return;
    }
    let total: usize = state.bin_counts.iter().sum();
    if total < 5 {
        return;
    }
    let n_total = total as f32;
    let sigma = config.sigma_x();
    if sigma <= 0.0 {
        return;
    }
    let r = config.particle_radius;
    let area_per_particle = PI * r * r;
    let inv_two_sigma_sq = 1.0 / (2.0 * sigma * sigma);
    let inv_sigma_sqrt_2pi = 1.0 / (sigma * (2.0 * PI).sqrt());
    let scale = n_total * area_per_particle / PACKING_EFFICIENCY;
    let baseline = dims.bin_bottom_y;

    let half_span = config.peg_spacing_x() * (config.rows as f32 * 0.5 + 0.5);
    let n_samples = 240;
    let mut prev: Option<Vec2> = None;
    let curve_color = Color::srgb(0.45, 1.0, 0.55);
    for k in 0..=n_samples {
        let t = k as f32 / n_samples as f32;
        let x = -half_span + 2.0 * half_span * t;
        let pdf = inv_sigma_sqrt_2pi * (-x * x * inv_two_sigma_sq).exp();
        let h = scale * pdf;
        let pt = Vec2::new(x, baseline + h);
        if let Some(p0) = prev {
            gizmos.line_2d(p0, pt, curve_color);
        }
        prev = Some(pt);
    }
    let marker_color = Color::srgba(0.45, 1.0, 0.55, 0.55);
    for &k in &[1.0, -1.0, 2.0, -2.0] {
        let x = k * sigma;
        let pdf = inv_sigma_sqrt_2pi * (-x * x * inv_two_sigma_sq).exp();
        let h = scale * pdf;
        gizmos.line_2d(
            Vec2::new(x, baseline),
            Vec2::new(x, baseline + h),
            marker_color,
        );
    }
}

fn draw_histogram(
    config: Res<BoardConfig>,
    dims: Res<BoardDims>,
    state: Res<SimState>,
    mut gizmos: Gizmos,
) {
    if !state.show_histogram_bars {
        return;
    }
    let r = config.particle_radius;
    let area = PI * r * r;
    let s = config.peg_spacing_x();
    let half_n = config.rows as f32 * 0.5;
    let baseline = dims.bin_bottom_y;
    let bar_color = Color::srgba(1.0, 0.55, 0.25, 0.75);
    for (i, &count) in state.bin_counts.iter().enumerate() {
        if count == 0 {
            continue;
        }
        let cx = (i as f32 - half_n) * s;
        let expected_h = (count as f32 * area) / (s * PACKING_EFFICIENCY);
        let x0 = cx - s * 0.45;
        let x1 = cx + s * 0.45;
        let y = baseline + expected_h;
        gizmos.line_2d(Vec2::new(x0, y), Vec2::new(x1, y), bar_color);
    }
}
