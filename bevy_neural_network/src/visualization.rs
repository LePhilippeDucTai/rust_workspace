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
use crate::config::{
    BOUND_RES, DATA_RECT, LOSS_RECT, NETWORK_RECT, WINDOW_HEIGHT, WINDOW_WIDTH, lerp,
};
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
            .add_systems(
                Startup,
                (setup_background, setup_boundary_sprite, setup_gizmo_config),
            )
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

/// Couleur émissive (HDR) d'un neurone selon son activation dans [-1, 1] :
/// sombre près de 0, et d'autant plus lumineux (donc « halo » via le bloom) que
/// l'activation est saturée — bleu si négative, orange si positive.
fn neuron_color(v: f32) -> Color {
    let m = v.abs().clamp(0.0, 1.0);
    let k = m * m;
    let hue = if v >= 0.0 {
        (1.0, 0.5, 0.18)
    } else {
        (0.32, 0.6, 1.0)
    };
    let base = 0.05;
    let glow = 2.8 * k;
    LinearRgba::new(
        base + hue.0 * glow,
        base + hue.1 * glow,
        base + hue.2 * glow,
        1.0,
    )
    .into()
}

/// Couleur émissive d'un poids : bleu si négatif, orange si positif. Plus le
/// poids est fort, plus l'arête est brillante et opaque (et rayonne).
fn weight_color(w: f32) -> Color {
    let m = (w.abs() / 1.2).clamp(0.0, 1.0);
    let g = 0.2 + 1.7 * m;
    let a = 0.10 + 0.6 * m;
    if w >= 0.0 {
        LinearRgba::new(1.0 * g, 0.45 * g, 0.16 * g, a).into()
    } else {
        LinearRgba::new(0.22 * g, 0.5 * g, 1.0 * g, a).into()
    }
}

/// Largeur des traits gizmos (arêtes, impulsions, courbes, cadres).
fn setup_gizmo_config(mut store: ResMut<GizmoConfigStore>) {
    let (config, _) = store.config_mut::<DefaultGizmoConfigGroup>();
    config.line.width = 2.2;
}

/// Fond en vignette radiale : centre légèrement éclairé, bords sombres, pour
/// donner de la profondeur à la scène.
fn setup_background(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let (w, h) = (192u32, 130u32);
    let mut image = Image::new_fill(
        Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    for py in 0..h {
        for px in 0..w {
            let nx = px as f32 / (w - 1) as f32 - 0.5;
            let ny = py as f32 / (h - 1) as f32 - 0.5;
            let d = ((nx * nx + ny * ny).sqrt() / 0.7071).clamp(0.0, 1.0);
            let v = 1.0 - d;
            let c = lerp_col((0.02, 0.02, 0.045), (0.10, 0.10, 0.17), v);
            let _ = image.set_color_at(px, py, c);
        }
    }
    let handle = images.add(image);
    commands.spawn((
        Sprite {
            image: handle,
            custom_size: Some(Vec2::new(WINDOW_WIDTH, WINDOW_HEIGHT)),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, -10.0),
    ));
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
            m.color = neuron_color(v);
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
    // Couleurs HDR (valeurs > 1) : les impulsions deviennent de véritables
    // traînées de lumière grâce au bloom.
    let fwd = LinearRgba::new(0.7, 3.0, 3.8, 1.0);
    let bwd = LinearRgba::new(3.8, 2.0, 0.7, 1.0);
    let r = neuron_radius(sizes);

    // Segment de couches actuellement traversé, et sens de l'impulsion.
    let (m, forward) = match anim.phase {
        Phase::Forward => (anim.seg.clamp(0, l as i32 - 2) as usize, true),
        Phase::Backward => {
            let s = anim.seg.clamp(0, l as i32 - 2) as usize;
            ((l - 2).saturating_sub(s), false)
        }
        Phase::Update => {
            // Flash : on illumine tous les neurones brièvement.
            let f = (1.0 - anim.t).max(0.0);
            let glow: Color = LinearRgba::new(2.6 * f, 2.6 * f, 3.0 * f, 1.0).into();
            for layer in 0..l {
                for idx in 0..sizes[layer] {
                    let p = neuron_pos(layer, idx, sizes);
                    gizmos.circle_2d(Isometry2d::from_translation(p), r + 4.0, glow);
                }
            }
            return;
        }
    };

    // Halo des neurones source du segment actif.
    let src = if forward { m } else { m + 1 };
    let glow: Color = LinearRgba::new(2.2, 2.4, 3.0, 1.0).into();
    for idx in 0..sizes[src] {
        let p = neuron_pos(src, idx, sizes);
        gizmos.circle_2d(Isometry2d::from_translation(p), r + 3.0, glow);
    }

    // Impulsions avec traînée (afterimage) le long des arêtes du segment.
    const TRAIL: usize = 5;
    for i in 0..sizes[m] {
        for j in 0..sizes[m + 1] {
            let a = neuron_pos(m, i, sizes);
            let b = neuron_pos(m + 1, j, sizes);
            for k in 0..TRAIL {
                let tt = anim.t - k as f32 * 0.06;
                if !(0.0..=1.0).contains(&tt) {
                    continue;
                }
                let fade = 1.0 - k as f32 / TRAIL as f32;
                let base = if forward { fwd } else { bwd };
                let col: Color =
                    LinearRgba::new(base.red * fade, base.green * fade, base.blue * fade, 1.0)
                        .into();
                let p = if forward {
                    a.lerp(b, tt)
                } else {
                    b.lerp(a, tt)
                };
                gizmos.circle_2d(Isometry2d::from_translation(p), 1.5 + 3.0 * fade, col);
            }
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
        let target_col: Color = LinearRgba::new(0.5, 1.6, 0.8, 1.0).into();
        gizmos.linestrip_2d(target_pts, target_col);

        let steps = 80;
        let pred_pts = (0..steps).map(|k| {
            let ix = lerp(-1.0, 1.0, k as f32 / (steps - 1) as f32);
            let o = net.0.forward_pure(&[ix])[0].clamp(-1.0, 1.0);
            data_to_world(ix, o)
        });
        let pred_col: Color = LinearRgba::new(3.0, 2.0, 0.7, 1.0).into();
        gizmos.linestrip_2d(pred_pts, pred_col);

        for s in &data.0.samples {
            let p = data_to_world(s.x[0], s.y[0].clamp(-1.0, 1.0));
            gizmos.circle_2d(Isometry2d::from_translation(p), 2.5, target_col);
        }
        return;
    }

    // Classification : points colorés par classe, échantillon actif entouré.
    let active = anim.active % data.0.samples.len().max(1);
    let col_a: Color = LinearRgba::new(0.4, 2.0, 2.6, 1.0).into();
    let col_b: Color = LinearRgba::new(2.6, 0.7, 1.6, 1.0).into();
    let ring: Color = LinearRgba::new(3.0, 3.0, 3.0, 1.0).into();
    for (k, s) in data.0.samples.iter().enumerate() {
        let p = data_to_world(s.x[0], s.x[1]);
        let col = if s.class == 0 { col_a } else { col_b };
        gizmos.circle_2d(Isometry2d::from_translation(p), 3.0, col);
        if k == active {
            gizmos.circle_2d(Isometry2d::from_translation(p), 7.0, ring);
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
    let loss_col: Color = LinearRgba::new(3.0, 2.4, 0.5, 1.0).into();
    gizmos.linestrip_2d(pts, loss_col);
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
