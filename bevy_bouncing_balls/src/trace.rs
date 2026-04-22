//! Traçage de trajectoire d'une balle + calcul de quantiles x/y.

use bevy::prelude::*;
use rand::seq::IteratorRandom;

use crate::components::{Ball, TraceStatsText};
use crate::config::MAX_RADIUS;
use crate::resources::{SimSettings, TraceData};

/// Nombre de segments affichés dans la traîne (les plus récents).
const MAX_TRAIL_DISPLAY: usize = 6_000;
const QUANTILE_LEVELS: [f32; 7] = [0.01, 0.05, 0.25, 0.50, 0.75, 0.95, 0.99];

pub struct TracePlugin;

impl Plugin for TracePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TraceData>()
            .add_systems(Startup, setup_trace_hud)
            .add_systems(
                Update,
                (
                    handle_trace_toggle,
                    record_trace_position,
                    draw_trace_trail,
                    update_trace_hud,
                ),
            );
    }
}

fn setup_trace_hud(mut commands: Commands) {
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: 13.0,
            ..default()
        },
        TextColor(Color::srgb(0.55, 0.95, 0.65)),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(8.0),
            left: Val::Px(10.0),
            ..default()
        },
        TraceStatsText,
    ));
}

fn handle_trace_toggle(
    keys: Res<ButtonInput<KeyCode>>,
    mut trace: ResMut<TraceData>,
    balls: Query<Entity, With<Ball>>,
    mut ball_q: Query<&mut Ball>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut commands: Commands,
) {
    if !keys.just_pressed(KeyCode::KeyT) {
        return;
    }
    if trace.active {
        trace.active = false;
    } else {
        // Valider la cible existante ou en choisir une nouvelle
        let valid = trace.target.filter(|&e| balls.contains(e));
        if valid.is_none() {
            trace.positions.clear();
            let mut rng = rand::thread_rng();
            trace.target = balls.iter().choose(&mut rng);

            // Redimensionner la balle sélectionnée : 3× le rayon max
            if let Some(target) = trace.target {
                let new_radius = MAX_RADIUS * 3.0;
                if let Ok(mut ball) = ball_q.get_mut(target) {
                    ball.radius = new_radius;
                    ball.mass = new_radius;
                }
                commands
                    .entity(target)
                    .insert(Mesh2d(meshes.add(Circle::new(new_radius))));
            }
        }
        trace.active = trace.target.is_some();
    }
}

fn record_trace_position(
    mut trace: ResMut<TraceData>,
    balls: Query<&Transform, With<Ball>>,
    settings: Res<SimSettings>,
) {
    if !trace.active || settings.paused {
        return;
    }
    let Some(target) = trace.target else { return };
    match balls.get(target) {
        Ok(tf) => trace.positions.push(tf.translation.truncate()),
        Err(_) => {
            trace.target = None;
            trace.active = false;
        }
    }
}

fn draw_trace_trail(trace: Res<TraceData>, mut gizmos: Gizmos) {
    if !trace.active || trace.positions.len() < 2 {
        return;
    }
    let n = trace.positions.len();
    let start = n.saturating_sub(MAX_TRAIL_DISPLAY);
    let display = &trace.positions[start..];
    let count = display.len();

    for i in 1..count {
        let t = i as f32 / count as f32;
        let alpha = t * 0.80 + 0.05;
        gizmos.line_2d(display[i - 1], display[i], Color::srgba(0.25, 0.85, 1.0, alpha));
    }
    if let Some(&pos) = display.last() {
        gizmos.circle_2d(pos, 7.0, Color::WHITE);
    }
}

// Cache local pour éviter de re-trier tout le vecteur à chaque frame.
#[derive(Default)]
struct QCache {
    last_n: usize,
    xq: [f32; 7],
    yq: [f32; 7],
}

fn update_trace_hud(
    trace: Res<TraceData>,
    mut text_q: Query<&mut Text, With<TraceStatsText>>,
    mut cache: Local<QCache>,
) {
    let Ok(mut text) = text_q.single_mut() else {
        return;
    };

    let n = trace.positions.len();
    let state = if trace.active { "ON " } else { "OFF" };

    if n == 0 {
        **text = format!("[T] Trace : {state}  —  appuyer sur T pour tracer une balle");
        return;
    }

    // Recalcul toutes les 120 nouvelles positions pour limiter le coût du tri.
    if n >= cache.last_n + 120 || cache.last_n == 0 {
        let mut xs: Vec<f32> = trace.positions.iter().map(|p| p.x).collect();
        let mut ys: Vec<f32> = trace.positions.iter().map(|p| p.y).collect();
        xs.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap());
        ys.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap());
        for (i, &p) in QUANTILE_LEVELS.iter().enumerate() {
            cache.xq[i] = quantile_sorted(&xs, p);
            cache.yq[i] = quantile_sorted(&ys, p);
        }
        cache.last_n = n;
    }

    let xq = &cache.xq;
    let yq = &cache.yq;
    **text = format!(
        "[T] Trace : {state}  |  {n} positions\n\
         Q  :    1%      5%     25%     50%     75%     95%     99%\n\
         X  : {:6.1}  {:6.1}  {:6.1}  {:6.1}  {:6.1}  {:6.1}  {:6.1}\n\
         Y  : {:6.1}  {:6.1}  {:6.1}  {:6.1}  {:6.1}  {:6.1}  {:6.1}",
        xq[0], xq[1], xq[2], xq[3], xq[4], xq[5], xq[6],
        yq[0], yq[1], yq[2], yq[3], yq[4], yq[5], yq[6],
    );
}

fn quantile_sorted(sorted: &[f32], p: f32) -> f32 {
    let n = sorted.len();
    if n == 0 {
        return 0.0;
    }
    let idx = p * (n - 1) as f32;
    let lo = idx.floor() as usize;
    let hi = (idx.ceil() as usize).min(n - 1);
    sorted[lo] + (idx - lo as f32) * (sorted[hi] - sorted[lo])
}
