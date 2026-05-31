//! Rendu : diagramme du réseau, impulsions forward/backward, frontière de
//! décision et courbe de loss.
//!
//! Choix d'implémentation : les neurones sont des entités-meshes (peu
//! nombreuses, colorées individuellement), tandis que les arêtes, les
//! impulsions, les courbes et les cadres sont tracés en mode immédiat avec des
//! gizmos — ils reflètent ainsi toujours l'état courant du réseau sans gestion
//! d'entités pour les O(n²) connexions.

use bevy::image::Image;
use bevy::math::Isometry2d;
use bevy::prelude::*;
use bevy::asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::components::{BoundarySprite, NeuronIndex, NeuronViz};
use crate::config::{BOUND_RES, DATA_RECT, LOSS_RECT, NETWORK_RECT, lerp};
use crate::dataset::DatasetKind;
use crate::layout::{neuron_pos, neuron_radius};
use crate::training::{Anim, Data, GraphDirty, LossHist, Net, Phase};

pub struct VisualizationPlugin;

/// Cadence de recalcul de la frontière de décision (en secondes).
#[derive(Resource)]
pub struct BoundaryTimer(pub f32);

impl Plugin for VisualizationPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(BoundaryTimer(0.0))
            .add_systems(Startup, setup_boundary_sprite)
            .add_systems(
                Update,
                (
                    rebuild_neurons,
                    update_neuron_colors,
                    update_boundary,
                    draw_edges,
                    draw_pulses,
                    draw_input_space,
                    draw_loss_curve,
                    draw_frames,
                ),
            );
    }
}

// ---------------------------------------------------------------------------
// Couleurs
// ---------------------------------------------------------------------------

fn lerp_col(a: (f32, f32, f32), b: (f32, f32, f32), t: f32) -> Color {
    Color::srgb(
        lerp(a.0, b.0, t),
        lerp(a.1, b.1, t),
        lerp(a.2, b.2, t),
    )
}

/// Couleur divergente d'une valeur dans [-1, 1] : bleu (négatif) → sombre (0)
/// → orange (positif). Sert aux activations des neurones.
fn val_color(v: f32) -> Color {
    let t = (v.clamp(-1.0, 1.0) + 1.0) / 2.0;
    if t < 0.5 {
        lerp_col((0.25, 0.55, 1.0), (0.12, 0.12, 0.16), t / 0.5)
    } else {
        lerp_col((0.12, 0.12, 0.16), (1.0, 0.55, 0.2), (t - 0.5) / 0.5)
    }
}

/// Couleur d'un poids : bleu si négatif, orange si positif, opacité ∝ |poids|.
fn weight_color(w: f32) -> Color {
    let m = (w.abs()).clamp(0.0, 1.5) / 1.5;
    let a = 0.05 + 0.55 * m;
    if w >= 0.0 {
        Color::srgba(1.0, 0.5, 0.2, a)
    } else {
        Color::srgba(0.3, 0.6, 1.0, a)
    }
}

// ---------------------------------------------------------------------------
// Neurones (entités)
// ---------------------------------------------------------------------------

fn rebuild_neurons(
    mut commands: Commands,
    mut graph_dirty: ResMut<GraphDirty>,
    net: Res<Net>,
    existing: Query<Entity, With<NeuronViz>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    if !graph_dirty.0 {
        return;
    }
    graph_dirty.0 = false;

    for e in existing.iter() {
        commands.entity(e).despawn();
    }

    let sizes = &net.0.sizes;
    let r = neuron_radius(sizes);
    let mesh = meshes.add(Circle::new(r));
    for layer in 0..sizes.len() {
        for idx in 0..sizes[layer] {
            let pos = neuron_pos(layer, idx, sizes);
            let mat = materials.add(Color::srgb(0.18, 0.18, 0.22));
            commands.spawn((
                Mesh2d(mesh.clone()),
                MeshMaterial2d(mat),
                Transform::from_translation(pos.extend(1.0)),
                NeuronViz,
                NeuronIndex { layer, idx },
            ));
        }
    }
}

fn update_neuron_colors(
    net: Res<Net>,
    q: Query<(&MeshMaterial2d<ColorMaterial>, &NeuronIndex)>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    for (mh, ni) in q.iter() {
        if ni.layer >= net.0.a.len() || ni.idx >= net.0.a[ni.layer].len() {
            continue;
        }
        let v = net.0.a[ni.layer][ni.idx];
        if let Some(m) = materials.get_mut(&mh.0) {
            m.color = val_color(v);
        }
    }
}

// ---------------------------------------------------------------------------
// Arêtes et impulsions (gizmos)
// ---------------------------------------------------------------------------

fn draw_edges(net: Res<Net>, mut gizmos: Gizmos) {
    let sizes = &net.0.sizes;
    for l in 0..sizes.len().saturating_sub(1) {
        for j in 0..sizes[l + 1] {
            for i in 0..sizes[l] {
                let a = neuron_pos(l, i, sizes);
                let b = neuron_pos(l + 1, j, sizes);
                gizmos.line_2d(a, b, weight_color(net.0.w[l][j][i]));
            }
        }
    }
}

fn draw_pulses(net: Res<Net>, anim: Res<Anim>, mut gizmos: Gizmos) {
    let sizes = &net.0.sizes;
    let l = sizes.len();
    if l < 2 {
        return;
    }
    let fwd = Color::srgb(0.4, 0.95, 1.0);
    let bwd = Color::srgb(1.0, 0.7, 0.3);

    // Segment de couches actuellement traversé, et sens de l'impulsion.
    let (m, forward) = match anim.phase {
        Phase::Forward => (anim.seg.clamp(0, l as i32 - 2) as usize, true),
        Phase::Backward => {
            let s = anim.seg.clamp(0, l as i32 - 2) as usize;
            ((l - 2).saturating_sub(s), false)
        }
        Phase::Update => {
            // Flash : on illumine tous les neurones brièvement.
            let glow = Color::srgba(1.0, 1.0, 1.0, 1.0 - anim.t);
            for layer in 0..l {
                for idx in 0..sizes[layer] {
                    let p = neuron_pos(layer, idx, sizes);
                    gizmos.circle_2d(Isometry2d::from_translation(p), neuron_radius(sizes) + 4.0, glow);
                }
            }
            return;
        }
    };

    // Surbrillance des neurones source du segment actif.
    let src = if forward { m } else { m + 1 };
    let glow = Color::srgb(1.0, 1.0, 1.0);
    for idx in 0..sizes[src] {
        let p = neuron_pos(src, idx, sizes);
        gizmos.circle_2d(Isometry2d::from_translation(p), neuron_radius(sizes) + 3.0, glow);
    }

    // Impulsions qui glissent le long des arêtes du segment.
    let col = if forward { fwd } else { bwd };
    for i in 0..sizes[m] {
        for j in 0..sizes[m + 1] {
            let a = neuron_pos(m, i, sizes);
            let b = neuron_pos(m + 1, j, sizes);
            let p = if forward {
                a.lerp(b, anim.t)
            } else {
                b.lerp(a, anim.t)
            };
            gizmos.circle_2d(Isometry2d::from_translation(p), 3.5, col);
        }
    }
}

// ---------------------------------------------------------------------------
// Frontière de décision (texture) et espace d'entrée (gizmos)
// ---------------------------------------------------------------------------

fn setup_boundary_sprite(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let mut image = Image::new_fill(
        Extent3d {
            width: BOUND_RES,
            height: BOUND_RES,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 0],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.texture_descriptor.label = Some("decision_boundary");
    let handle = images.add(image);

    let size = DATA_RECT.size();
    let center = DATA_RECT.center();
    commands.spawn((
        Sprite {
            image: handle,
            custom_size: Some(size),
            ..default()
        },
        Transform::from_translation(center.extend(-1.0)),
        BoundarySprite,
    ));
}

fn update_boundary(
    time: Res<Time>,
    mut timer: ResMut<BoundaryTimer>,
    net: Res<Net>,
    data: Res<Data>,
    sprite_q: Query<&Sprite, With<BoundarySprite>>,
    mut images: ResMut<Assets<Image>>,
) {
    timer.0 -= time.delta_secs();
    if timer.0 > 0.0 {
        return;
    }
    timer.0 = 0.2;

    let Ok(sprite) = sprite_q.single() else {
        return;
    };
    let Some(image) = images.get_mut(&sprite.image) else {
        return;
    };

    let res = BOUND_RES;
    if data.0.is_regression() {
        // Pas de frontière 2D en régression : on rend le fond transparent.
        for py in 0..res {
            for px in 0..res {
                let _ = image.set_color_at(px, py, Color::srgba(0.0, 0.0, 0.0, 0.0));
            }
        }
        return;
    }

    let col_a = (0.10, 0.45, 0.55);
    let col_b = (0.65, 0.18, 0.45);
    for py in 0..res {
        let iy = lerp(1.0, -1.0, py as f32 / (res - 1) as f32);
        for px in 0..res {
            let ix = lerp(-1.0, 1.0, px as f32 / (res - 1) as f32);
            let o = net.0.forward_pure(&[ix, iy])[0].clamp(0.0, 1.0);
            let c = lerp_col(col_a, col_b, o);
            let srgb = c.to_srgba();
            let _ = image.set_color_at(
                px,
                py,
                Color::srgba(srgb.red, srgb.green, srgb.blue, 0.85),
            );
        }
    }
}

/// Map un point de l'espace d'entrée [-1, 1] vers le rectangle d'affichage.
fn data_to_world(ix: f32, iy: f32) -> Vec2 {
    Vec2::new(
        lerp(DATA_RECT.min.x, DATA_RECT.max.x, (ix + 1.0) / 2.0),
        lerp(DATA_RECT.min.y, DATA_RECT.max.y, (iy + 1.0) / 2.0),
    )
}

fn draw_input_space(net: Res<Net>, data: Res<Data>, anim: Res<Anim>, mut gizmos: Gizmos) {
    if data.0.is_regression() {
        // Courbe cible (échantillons) vs courbe prédite par le réseau.
        let target_pts = data.0.samples.iter().map(|s| {
            let v = s.y[0].clamp(-1.0, 1.0);
            data_to_world(s.x[0], v)
        });
        gizmos.linestrip_2d(target_pts, Color::srgb(0.4, 0.8, 0.5));

        let steps = 80;
        let pred_pts = (0..steps).map(|k| {
            let ix = lerp(-1.0, 1.0, k as f32 / (steps - 1) as f32);
            let o = net.0.forward_pure(&[ix])[0].clamp(-1.0, 1.0);
            data_to_world(ix, o)
        });
        gizmos.linestrip_2d(pred_pts, Color::srgb(1.0, 0.75, 0.3));

        for s in &data.0.samples {
            let p = data_to_world(s.x[0], s.y[0].clamp(-1.0, 1.0));
            gizmos.circle_2d(Isometry2d::from_translation(p), 2.5, Color::srgb(0.5, 0.9, 0.6));
        }
        return;
    }

    // Classification : points colorés par classe, échantillon actif entouré.
    let active = anim.active % data.0.samples.len().max(1);
    for (k, s) in data.0.samples.iter().enumerate() {
        let p = data_to_world(s.x[0], s.x[1]);
        let col = if s.class == 0 {
            Color::srgb(0.35, 0.95, 1.0)
        } else {
            Color::srgb(1.0, 0.5, 0.8)
        };
        gizmos.circle_2d(Isometry2d::from_translation(p), 3.0, col);
        if k == active {
            gizmos.circle_2d(Isometry2d::from_translation(p), 6.5, Color::WHITE);
        }
    }
}

// ---------------------------------------------------------------------------
// Courbe de loss et cadres
// ---------------------------------------------------------------------------

fn draw_loss_curve(loss: Res<LossHist>, mut gizmos: Gizmos) {
    let rect = LOSS_RECT;
    if loss.0.len() < 2 {
        return;
    }
    let n = loss.0.len();
    let max_v = loss.0.iter().copied().fold(1e-6_f32, f32::max);
    let pts = loss.0.iter().enumerate().map(|(k, &v)| {
        let x = lerp(rect.min.x, rect.max.x, k as f32 / (n - 1) as f32);
        let y = lerp(rect.min.y, rect.max.y, (v / max_v).clamp(0.0, 1.0));
        Vec2::new(x, y)
    });
    gizmos.linestrip_2d(pts, Color::srgb(0.95, 0.85, 0.3));
}

fn draw_frames(data: Res<Data>, mut gizmos: Gizmos) {
    let frame = Color::srgba(0.5, 0.5, 0.6, 0.5);
    for rect in [NETWORK_RECT, DATA_RECT, LOSS_RECT] {
        gizmos.rect_2d(
            Isometry2d::from_translation(rect.center()),
            rect.size(),
            frame,
        );
    }
    // Repère central de l'espace d'entrée (classification uniquement).
    if data.0.kind != DatasetKind::Regression {
        let c = DATA_RECT.center();
        let s = DATA_RECT.size();
        let axis = Color::srgba(0.6, 0.6, 0.7, 0.25);
        gizmos.line_2d(
            Vec2::new(c.x - s.x / 2.0, c.y),
            Vec2::new(c.x + s.x / 2.0, c.y),
            axis,
        );
        gizmos.line_2d(
            Vec2::new(c.x, c.y - s.y / 2.0),
            Vec2::new(c.x, c.y + s.y / 2.0),
            axis,
        );
    }
}
