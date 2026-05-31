//! Visualisation : nœuds, arêtes du graphe, histogramme.

use bevy::prelude::*;

use crate::components::{HistBar, HistTarget, NodeLabel, NodeMarker};
use crate::config::{HIST_BOTTOM_Y, HIST_CENTER_X, HIST_HEIGHT, HIST_WIDTH, NODE_RADIUS};
use crate::resources::{Chain, EmpiricalDistribution, NodePositions};
use crate::simulation::state_color;

pub struct VisualizationPlugin;

impl Plugin for VisualizationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (setup_nodes, setup_histogram).chain())
            .add_systems(Update, (draw_edges, update_histogram_bars));
    }
}

/// Place les nœuds du graphe avec leur étiquette.
fn setup_nodes(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    positions: Res<NodePositions>,
) {
    for (i, pos) in positions.0.iter().enumerate() {
        let color = state_color(i);

        // Cercle extérieur (anneau) plus foncé pour contraste.
        commands.spawn((
            Mesh2d(meshes.add(Circle::new(NODE_RADIUS + 3.0))),
            MeshMaterial2d(materials.add(Color::srgb(0.12, 0.12, 0.16))),
            Transform::from_xyz(pos.x, pos.y, 0.5),
        ));

        commands.spawn((
            Mesh2d(meshes.add(Circle::new(NODE_RADIUS))),
            MeshMaterial2d(materials.add(color)),
            Transform::from_xyz(pos.x, pos.y, 1.0),
            NodeMarker,
        ));

        commands.spawn((
            Text2d::new(format!("{i}")),
            TextFont {
                font_size: 26.0,
                ..default()
            },
            TextColor(Color::srgb(0.05, 0.05, 0.08)),
            Transform::from_xyz(pos.x, pos.y, 1.5),
            NodeLabel,
        ));
    }
}

/// Dessine les arêtes (probabilités de transition) via Gizmos chaque frame.
fn draw_edges(mut gizmos: Gizmos, chain: Res<Chain>, positions: Res<NodePositions>) {
    let n = chain.0.num_states();
    for i in 0..n {
        for j in 0..n {
            let p = chain.0.transition[[i, j]];
            if p < 1e-3 || i == j {
                continue;
            }
            let from = positions.0[i];
            let to = positions.0[j];

            // Couleur de l'arête : opacité proportionnelle à la probabilité.
            let alpha = (0.20 + 0.80 * p as f32).clamp(0.0, 1.0);
            let color = Color::srgba(0.75, 0.75, 0.85, alpha);

            // Décale légèrement l'arête perpendiculairement pour séparer
            // visuellement les deux directions (i→j vs j→i).
            let dir = (to - from).normalize_or_zero();
            let perp = Vec2::new(-dir.y, dir.x) * 6.0;
            let a = from + dir * NODE_RADIUS + perp;
            let b = to - dir * NODE_RADIUS + perp;

            gizmos.line_2d(a, b, color);

            // Petite "flèche" à l'extrémité.
            let arrow_back = b - dir * 10.0;
            gizmos.line_2d(b, arrow_back + perp.normalize_or_zero() * 4.0, color);
            gizmos.line_2d(b, arrow_back - perp.normalize_or_zero() * 4.0, color);
        }
    }
}

/// Met en place l'histogramme : barres empiriques + marqueurs de la
/// distribution stationnaire théorique π.
fn setup_histogram(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    chain: Res<Chain>,
) {
    let n = chain.0.num_states();
    let bar_w = HIST_WIDTH / (n as f32 * 1.4);
    let gap = (HIST_WIDTH - bar_w * n as f32) / (n as f32 + 1.0);

    // Cadre (fond) de l'histogramme.
    let frame_x = HIST_CENTER_X;
    let frame_y = HIST_BOTTOM_Y + HIST_HEIGHT * 0.5;
    commands.spawn((
        Mesh2d(meshes.add(Rectangle::new(HIST_WIDTH + 20.0, HIST_HEIGHT + 40.0))),
        MeshMaterial2d(materials.add(Color::srgba(0.15, 0.15, 0.20, 0.55))),
        Transform::from_xyz(frame_x, frame_y + 10.0, -0.5),
    ));

    // Titre.
    commands.spawn((
        Text2d::new("Distribution: empirique vs stationnaire pi"),
        TextFont {
            font_size: 16.0,
            ..default()
        },
        TextColor(Color::srgb(0.92, 0.92, 0.96)),
        Transform::from_xyz(frame_x, HIST_BOTTOM_Y + HIST_HEIGHT + 28.0, 0.0),
    ));

    for i in 0..n {
        let x = HIST_CENTER_X - HIST_WIDTH * 0.5 + gap + (i as f32) * (bar_w + gap) + bar_w * 0.5;
        let color = state_color(i);

        // Barre empirique (hauteur ajustée dynamiquement).
        commands.spawn((
            Mesh2d(meshes.add(Rectangle::new(bar_w, 1.0))),
            MeshMaterial2d(materials.add(color)),
            Transform::from_xyz(x, HIST_BOTTOM_Y, 0.0),
            HistBar { state: i },
        ));

        // Marqueur de la distribution stationnaire (trait horizontal rouge).
        let pi = chain.0.stationary[i] as f32;
        let target_y = HIST_BOTTOM_Y + pi * HIST_HEIGHT;
        commands.spawn((
            Mesh2d(meshes.add(Rectangle::new(bar_w + 6.0, 3.0))),
            MeshMaterial2d(materials.add(Color::srgba(0.95, 0.20, 0.20, 0.95))),
            Transform::from_xyz(x, target_y, 0.3),
            HistTarget,
        ));

        // Étiquette d'état sous la barre.
        commands.spawn((
            Text2d::new(format!("{i}")),
            TextFont {
                font_size: 14.0,
                ..default()
            },
            TextColor(Color::srgb(0.85, 0.85, 0.92)),
            Transform::from_xyz(x, HIST_BOTTOM_Y - 14.0, 0.0),
        ));

        // Valeur numérique de π_i.
        commands.spawn((
            Text2d::new(format!("π={:.3}", pi)),
            TextFont {
                font_size: 11.0,
                ..default()
            },
            TextColor(Color::srgba(0.95, 0.40, 0.40, 1.0)),
            Transform::from_xyz(x, HIST_BOTTOM_Y - 28.0, 0.0),
        ));
    }

    // Légende.
    commands.spawn((
        Text2d::new("rouge = pi theorique (distribution stationnaire)"),
        TextFont {
            font_size: 11.0,
            ..default()
        },
        TextColor(Color::srgba(0.95, 0.40, 0.40, 1.0)),
        Transform::from_xyz(frame_x, HIST_BOTTOM_Y - 48.0, 0.0),
    ));
}

/// Met à jour les barres empiriques en fonction de la distribution courante.
fn update_histogram_bars(
    empirical: Res<EmpiricalDistribution>,
    mut bars: Query<(&HistBar, &mut Transform)>,
) {
    if empirical.total < 1.0 {
        return;
    }
    for (bar, mut tf) in bars.iter_mut() {
        let p = empirical.counts[bar.state] / empirical.total;
        let h = (p * HIST_HEIGHT).max(0.5);
        tf.scale.y = h;
        tf.translation.y = HIST_BOTTOM_Y + h * 0.5;
    }
}
