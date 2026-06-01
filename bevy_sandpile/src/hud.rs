//! HUD : contrôles, statistiques de criticité, et histogramme log-log des
//! tailles d'avalanches (la signature de la loi de puissance).

use bevy::prelude::*;

use crate::components::{HudText, StatsText};
use crate::config::{
    CELL_PX, GRID_CX, GRID_CY, GRID_H, GRID_W, HIST_BOTTOM, HIST_LEFT, HIST_RIGHT, HIST_TOP,
};
use crate::resources::{AvalancheHistogram, Grid, SimSettings, Stats};

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_hud)
            .add_systems(Update, (update_hud, draw_overlay));
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
    grid: Res<Grid>,
    hist: Res<AvalancheHistogram>,
    time: Res<Time>,
    mut hud_q: Query<&mut Text, (With<HudText>, Without<StatsText>)>,
    mut stats_q: Query<&mut Text, (With<StatsText>, Without<HudText>)>,
) {
    let fps = 1.0 / time.delta_secs().max(1e-6);

    if let Ok(mut text) = hud_q.single_mut() {
        **text = format!(
            "Tas de sable abelien (Bak-Tang-Wiesenfeld) - criticite auto-organisee\n\
             Seuil 4 : une cellule a 4+ grains s'effondre et en donne 1 a chaque voisin (bord = dissipation)\n\
             FPS: {fps:.0}   [Space] pause: {}   [M] mode: {}   [Up/Down] grains/frame: {}\n\
             [R] reset grille+stats   [C] vider l'histogramme",
            if settings.paused { "ON" } else { "OFF" },
            settings.mode.label(),
            settings.drops_per_frame,
        );
    }

    if let Ok(mut text) = stats_q.single_mut() {
        let cells = (GRID_W * GRID_H) as f64;
        let density = grid.pile.mass() as f64 / cells;
        let mean_av = if stats.avalanches > 0 {
            stats.total_topplings as f64 / stats.avalanches as f64
        } else {
            0.0
        };
        let slope_txt = match hist.fit_slope() {
            Some(s) => format!("pente log-log: {s:.2}  ->  exposant tau ~ {:.2}", -s),
            None => "pente log-log: (pas encore assez de donnees)".to_string(),
        };
        **text = format!(
            "Grains ajoutes: {}   Avalanches: {}   (depots sans effondrement: {})\n\
             Densite moyenne: {density:.3} grains/cellule  (se cale spontanement vers ~2.1 au seuil critique)\n\
             Avalanche moyenne: {mean_av:.1}   max: {}   derniere: {}\n\
             Loi de puissance P(s) ~ s^(-tau)  =>  {slope_txt}",
            stats.grains_added,
            stats.avalanches,
            stats.zero_avalanches,
            stats.max_avalanche,
            stats.last_avalanche,
        );
    }
}

/// Dessine le cadre de la grille et le nuage log-log de l'histogramme.
fn draw_overlay(mut gizmos: Gizmos, hist: Res<AvalancheHistogram>) {
    // Cadre de la grille.
    let hw = GRID_W as f32 * CELL_PX * 0.5;
    let hh = GRID_H as f32 * CELL_PX * 0.5;
    let c = Vec2::new(GRID_CX, GRID_CY);
    let border = Color::srgba(0.5, 0.5, 0.62, 0.7);
    gizmos.rect_2d(c, Vec2::new(hw * 2.0, hh * 2.0), border);

    // Cadre du panneau histogramme.
    let axis = Color::srgba(0.7, 0.7, 0.8, 0.8);
    let bl = Vec2::new(HIST_LEFT, HIST_BOTTOM);
    let br = Vec2::new(HIST_RIGHT, HIST_BOTTOM);
    let tl = Vec2::new(HIST_LEFT, HIST_TOP);
    gizmos.line_2d(bl, br, axis); // axe horizontal : log10(taille)
    gizmos.line_2d(bl, tl, axis); // axe vertical : log10(densite)

    let pts = hist.log_log_points();
    if pts.len() < 2 {
        return;
    }

    // Bornes des données pour la mise à l'échelle dans le panneau.
    let (mut xmin, mut xmax) = (f64::INFINITY, f64::NEG_INFINITY);
    let (mut ymin, mut ymax) = (f64::INFINITY, f64::NEG_INFINITY);
    for &(x, y) in &pts {
        xmin = xmin.min(x);
        xmax = xmax.max(x);
        ymin = ymin.min(y);
        ymax = ymax.max(y);
    }
    let xspan = (xmax - xmin).max(1e-6);
    let yspan = (ymax - ymin).max(1e-6);

    let pad = 14.0_f32;
    let map = |x: f64, y: f64| -> Vec2 {
        let fx = ((x - xmin) / xspan) as f32;
        let fy = ((y - ymin) / yspan) as f32;
        Vec2::new(
            HIST_LEFT + pad + fx * (HIST_RIGHT - HIST_LEFT - 2.0 * pad),
            HIST_BOTTOM + pad + fy * (HIST_TOP - HIST_BOTTOM - 2.0 * pad),
        )
    };

    // Points de l'histogramme (jaune) reliés pour suivre la décroissance.
    let dot = Color::srgb(0.98, 0.85, 0.3);
    let mut prev: Option<Vec2> = None;
    for &(x, y) in &pts {
        let p = map(x, y);
        gizmos.circle_2d(p, 2.0, dot);
        if let Some(q) = prev {
            gizmos.line_2d(q, p, Color::srgba(0.98, 0.85, 0.3, 0.35));
        }
        prev = Some(p);
    }

    // Droite d'ajustement (cyan) : une ligne droite ici = loi de puissance.
    if let Some(slope) = hist.fit_slope() {
        let n = pts.len() as f64;
        let mx = pts.iter().map(|p| p.0).sum::<f64>() / n;
        let my = pts.iter().map(|p| p.1).sum::<f64>() / n;
        let intercept = my - slope * mx;
        let y_at = |x: f64| slope * x + intercept;
        let a = map(xmin, y_at(xmin));
        let b = map(xmax, y_at(xmax));
        gizmos.line_2d(a, b, Color::srgb(0.3, 0.95, 1.0));
    }
}
