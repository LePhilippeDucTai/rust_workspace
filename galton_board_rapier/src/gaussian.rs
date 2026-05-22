//! Comptage par bac + tracé de la densité gaussienne théorique.
//! La mesure se fait sur les `Transform` des particules, peu importe qui les a déplacées.

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
    state.bin_counts = vec![0; nb];
    let (s, half_n, bin_top) = (
        config.peg_spacing_x(),
        config.rows as f32 * 0.5,
        dims.bin_top_y,
    );
    for tf in &particles {
        if tf.translation.y > bin_top {
            continue;
        }
        let idx = (tf.translation.x / s + half_n).round() as i32;
        if idx >= 0 && (idx as usize) < nb {
            state.bin_counts[idx as usize] += 1;
        }
    }
}

fn gaussian_height(x: f32, scale: f32, itss: f32, isq2pi: f32) -> f32 {
    scale * isq2pi * (-x * x * itss).exp()
}

fn draw_curve(
    gizmos: &mut Gizmos,
    half_span: f32,
    baseline: f32,
    scale: f32,
    itss: f32,
    isq2pi: f32,
) {
    let mut prev: Option<Vec2> = None;
    let steps = 300u32;
    for k in 0..=steps {
        let t = k as f32 / steps as f32;
        let x = -half_span + 2.0 * half_span * t;
        let hue = 120.0 + 180.0 * t;
        let color = Color::hsla(hue, 1.0, 0.65, 0.92);
        let pt = Vec2::new(x, baseline + gaussian_height(x, scale, itss, isq2pi));
        if let Some(p0) = prev {
            gizmos.line_2d(p0, pt, color);
        }
        prev = Some(pt);
    }
}

fn draw_sigma_markers(
    gizmos: &mut Gizmos,
    sigma: f32,
    baseline: f32,
    scale: f32,
    itss: f32,
    isq2pi: f32,
) {
    for &k in &[1.0f32, -1.0, 2.0, -2.0] {
        let color = if k.abs() < 1.5 {
            Color::srgba(0.95, 0.95, 0.30, 0.70)
        } else {
            Color::srgba(0.95, 0.55, 0.20, 0.55)
        };
        let x = k * sigma;
        let h = gaussian_height(x, scale, itss, isq2pi);
        gizmos.line_2d(Vec2::new(x, baseline), Vec2::new(x, baseline + h), color);
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
    // Normalisation fixe : la densité théorique est précalculable.
    // On utilise target_particles (connu dès le départ) plutôt que le nombre
    // de balles déjà posées, ce qui évite que la courbe se déforme pendant
    // la chute et permet de voir immédiatement si l'histogramme converge.
    let n_counted: usize = state.bin_counts.iter().sum();
    if n_counted < 5 {
        return;
    }
    let sigma = config.sigma_x();
    let r = config.particle_radius;
    let (itss, isq2pi) = (
        1.0 / (2.0 * sigma * sigma),
        1.0 / (sigma * (2.0 * PI).sqrt()),
    );
    let scale = n_counted as f32 * PI * r * r / PACKING_EFFICIENCY;
    let half_span = config.peg_spacing_x() * (config.rows as f32 * 0.5 + 0.5);
    draw_curve(
        &mut gizmos,
        half_span,
        dims.bin_bottom_y,
        scale,
        itss,
        isq2pi,
    );
    draw_sigma_markers(&mut gizmos, sigma, dims.bin_bottom_y, scale, itss, isq2pi);
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
    let (r, s) = (config.particle_radius, config.peg_spacing_x());
    let (half_n, baseline) = (config.rows as f32 * 0.5, dims.bin_bottom_y);
    let area = PI * r * r;
    let nb = state.bin_counts.len();
    for (i, &count) in state.bin_counts.iter().enumerate() {
        if count == 0 {
            continue;
        }
        let cx = (i as f32 - half_n) * s;
        let y = baseline + count as f32 * area / (s * PACKING_EFFICIENCY);
        let hue = 200.0 + 120.0 * i as f32 / nb.max(1) as f32;
        let color = Color::hsla(hue, 1.0, 0.65, 0.80);
        gizmos.line_2d(
            Vec2::new(cx - s * 0.44, baseline),
            Vec2::new(cx - s * 0.44, y),
            color,
        );
        gizmos.line_2d(
            Vec2::new(cx + s * 0.44, baseline),
            Vec2::new(cx + s * 0.44, y),
            color,
        );
        gizmos.line_2d(
            Vec2::new(cx - s * 0.44, y),
            Vec2::new(cx + s * 0.44, y),
            color,
        );
    }
}
