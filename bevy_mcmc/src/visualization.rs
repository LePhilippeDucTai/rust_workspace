//! Rendu : heatmap de la cible, marqueurs de la chaîne, trace de la marche.

use bevy::prelude::*;

use crate::components::{CurrentMarker, HeatCell, MeanMarker, ProposalGhost};
use crate::config::{
    DOMAIN_MAX, DOMAIN_MIN, HEAT_RES, PLOT_CX, PLOT_CY, PLOT_HALF, target_to_screen,
};
use crate::resources::{ChainState, SimSettings, Stats, TraceBuffer};
use crate::target::Target;

pub struct VisualizationPlugin;

impl Plugin for VisualizationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (setup_heatmap, setup_markers))
            .add_systems(
                Update,
                (toggle_heatmap, update_markers, draw_overlay),
            );
    }
}

/// Palette type « inferno » : interpolation entre quelques arrêts.
fn colormap(t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    const STOPS: [(f32, f32, f32, f32); 6] = [
        (0.00, 0.05, 0.03, 0.12),
        (0.20, 0.22, 0.08, 0.40),
        (0.40, 0.48, 0.12, 0.42),
        (0.60, 0.78, 0.25, 0.30),
        (0.80, 0.96, 0.55, 0.15),
        (1.00, 0.99, 0.92, 0.45),
    ];
    let mut lo = STOPS[0];
    let mut hi = STOPS[STOPS.len() - 1];
    for w in STOPS.windows(2) {
        if t >= w[0].0 && t <= w[1].0 {
            lo = w[0];
            hi = w[1];
            break;
        }
    }
    let span = (hi.0 - lo.0).max(1e-6);
    let f = (t - lo.0) / span;
    Color::srgb(
        lo.1 + (hi.1 - lo.1) * f,
        lo.2 + (hi.2 - lo.2) * f,
        lo.3 + (hi.3 - lo.3) * f,
    )
}

/// Construit la heatmap statique de la densité cible π(x).
fn setup_heatmap(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    target: Res<Target>,
) {
    let cell = (PLOT_HALF * 2.0) / HEAT_RES as f32;
    let span = DOMAIN_MAX - DOMAIN_MIN;
    let cell_mesh = meshes.add(Rectangle::new(cell, cell));

    for i in 0..HEAT_RES {
        for j in 0..HEAT_RES {
            let x = DOMAIN_MIN + span * (i as f32 + 0.5) / HEAT_RES as f32;
            let y = DOMAIN_MIN + span * (j as f32 + 0.5) / HEAT_RES as f32;
            let d = target.density(Vec2::new(x, y));
            // Racine pour faire ressortir les zones de faible densité.
            let t = (d / target.max_density).clamp(0.0, 1.0).powf(0.5);
            let pos = target_to_screen(Vec2::new(x, y));
            commands.spawn((
                Mesh2d(cell_mesh.clone()),
                MeshMaterial2d(materials.add(colormap(t))),
                Transform::from_xyz(pos.x, pos.y, 0.0),
                HeatCell,
            ));
        }
    }
}

/// Prépare les marqueurs mobiles (état courant, proposition, moyenne).
fn setup_markers(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    // Fantôme de la proposition (anneau pâle).
    commands.spawn((
        Mesh2d(meshes.add(Circle::new(6.0))),
        MeshMaterial2d(materials.add(Color::srgba(0.9, 0.9, 0.95, 0.45))),
        Transform::from_xyz(0.0, 0.0, 8.0),
        ProposalGhost,
    ));

    // État courant de la chaîne (point cyan vif).
    commands.spawn((
        Mesh2d(meshes.add(Circle::new(7.0))),
        MeshMaterial2d(materials.add(Color::srgb(0.25, 0.95, 1.0))),
        Transform::from_xyz(0.0, 0.0, 10.0),
        CurrentMarker,
    ));

    // Moyenne empirique courante (point magenta) — converge vers E[X].
    commands.spawn((
        Mesh2d(meshes.add(Circle::new(6.0))),
        MeshMaterial2d(materials.add(Color::srgb(1.0, 0.25, 0.85))),
        Transform::from_xyz(0.0, 0.0, 9.0),
        MeanMarker,
    ));
}

/// Affiche / masque la heatmap selon le réglage.
fn toggle_heatmap(
    settings: Res<SimSettings>,
    mut cells: Query<&mut Visibility, With<HeatCell>>,
) {
    let target_vis = if settings.show_heatmap {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut vis in cells.iter_mut() {
        if *vis != target_vis {
            *vis = target_vis;
        }
    }
}

/// Place les marqueurs mobiles à leur position courante.
#[allow(clippy::type_complexity)]
fn update_markers(
    chain: Res<ChainState>,
    stats: Res<Stats>,
    mut sets: ParamSet<(
        Query<&mut Transform, With<CurrentMarker>>,
        Query<&mut Transform, With<ProposalGhost>>,
        Query<(&mut Transform, &mut Visibility), With<MeanMarker>>,
    )>,
) {
    let cur = target_to_screen(chain.current);
    if let Ok(mut tf) = sets.p0().single_mut() {
        tf.translation.x = cur.x;
        tf.translation.y = cur.y;
    }

    let prop = target_to_screen(chain.last_proposal);
    if let Ok(mut tf) = sets.p1().single_mut() {
        tf.translation.x = prop.x;
        tf.translation.y = prop.y;
    }

    if let Ok((mut tf, mut vis)) = sets.p2().single_mut() {
        if stats.recorded > 0 {
            let m = target_to_screen(stats.empirical_mean());
            tf.translation.x = m.x;
            tf.translation.y = m.y;
            *vis = Visibility::Inherited;
        } else {
            *vis = Visibility::Hidden;
        }
    }
}

/// Dessine au gizmo : bordure du tracé, vraie moyenne, modes, trace, lien
/// état→proposition.
fn draw_overlay(
    mut gizmos: Gizmos,
    settings: Res<SimSettings>,
    target: Res<Target>,
    chain: Res<ChainState>,
    trace: Res<TraceBuffer>,
) {
    // Bordure de la zone de tracé.
    let h = PLOT_HALF;
    let c = Vec2::new(PLOT_CX, PLOT_CY);
    let border = Color::srgba(0.6, 0.6, 0.7, 0.6);
    let bl = c + Vec2::new(-h, -h);
    let br = c + Vec2::new(h, -h);
    let tl = c + Vec2::new(-h, h);
    let tr = c + Vec2::new(h, h);
    gizmos.line_2d(bl, br, border);
    gizmos.line_2d(br, tr, border);
    gizmos.line_2d(tr, tl, border);
    gizmos.line_2d(tl, bl, border);

    // Modes théoriques (centres des gaussiennes) : petites croix blanches.
    for comp in &target.comps {
        let p = target_to_screen(comp.mean);
        let s = 6.0;
        let col = Color::srgba(1.0, 1.0, 1.0, 0.7);
        gizmos.line_2d(p + Vec2::new(-s, 0.0), p + Vec2::new(s, 0.0), col);
        gizmos.line_2d(p + Vec2::new(0.0, -s), p + Vec2::new(0.0, s), col);
    }

    // Vraie moyenne E[X] : grande croix verte (cible de convergence).
    let tm = target_to_screen(target.mean);
    let s = 11.0;
    let green = Color::srgb(0.4, 1.0, 0.45);
    gizmos.line_2d(tm + Vec2::new(-s, 0.0), tm + Vec2::new(s, 0.0), green);
    gizmos.line_2d(tm + Vec2::new(0.0, -s), tm + Vec2::new(0.0, s), green);

    // Trace de la marche aléatoire (segments récents, plus pâles vers le passé).
    if settings.show_trace && trace.points.len() >= 2 {
        let n = trace.points.len();
        for k in 0..n - 1 {
            let a = target_to_screen(trace.points[k]);
            let b = target_to_screen(trace.points[k + 1]);
            let alpha = 0.15 + 0.75 * (k as f32 / (n as f32 - 1.0));
            gizmos.line_2d(a, b, Color::srgba(0.3, 0.95, 1.0, alpha));
        }
    }

    // Lien état courant → dernière proposition.
    let cur = target_to_screen(chain.current);
    let prop = target_to_screen(chain.last_proposal);
    let link = if chain.last_accepted {
        Color::srgba(0.4, 1.0, 0.5, 0.9)
    } else {
        Color::srgba(1.0, 0.4, 0.4, 0.9)
    };
    gizmos.line_2d(cur, prop, link);
}
