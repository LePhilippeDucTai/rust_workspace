//! Mouvement brownien 2D — un nuage de marcheurs aléatoires diffusant depuis
//! l'origine.
//!
//! Choix de visualisation : la difficulté d'une marche brownienne « brute »
//! est que son amplitude (en sqrt(t)) est minuscule comparée à la fenêtre, donc
//! invisible. On résout ça avec un *auto-zoom* : les positions réelles de
//! simulation sont stockées dans les composants `Particle`, et chaque frame on
//! les projette à l'écran via une échelle dynamique qui maintient le nuage
//! toujours visible, quelle que soit l'étendue atteinte.
//!
//! On affiche aussi le rayon théorique RMS = 2·sqrt(D·t) (cercle d'enveloppe)
//! et la valeur mesurée, pour vérifier visuellement la loi de diffusion.

use bevy::math::Isometry2d;
use bevy::prelude::*;
use rand_distr::{Distribution, Normal};
use std::collections::VecDeque;

// ---------------------------------------------------------------------------
// Constantes
// ---------------------------------------------------------------------------

const WINDOW_WIDTH: f32 = 1280.0;
const WINDOW_HEIGHT: f32 = 960.0;

/// Nombre de marcheurs dans le nuage diffusif.
const N_PARTICLES: usize = 1600;
/// Marcheurs « traceurs » qui laissent une traînée lumineuse.
const N_TRACERS: usize = 7;
/// Taille de la palette de couleurs partagée (matériaux réutilisés).
const PALETTE_SIZE: usize = 64;

/// Coefficient de diffusion (px²/s). Fixe le rythme d'expansion.
const DIFFUSION: f32 = 3500.0;
/// Longueur maximale d'une traînée de traceur (en points).
const MAX_TRAIL: usize = 650;

/// Rayon visuel (px écran) des points du nuage et des traceurs.
const DOT_RADIUS: f32 = 2.2;
const TRACER_RADIUS: f32 = 4.5;

/// Fraction de la demi-fenêtre que l'on cherche à remplir avec 3·RMS.
const VIEW_FILL: f32 = 0.84;
/// Bornes de l'échelle d'affichage (zoom).
const SCALE_MIN: f32 = 0.02;
const SCALE_MAX: f32 = 9.0;

// ---------------------------------------------------------------------------
// Composants
// ---------------------------------------------------------------------------

/// Un marcheur. `pos` est la position *réelle* de simulation (unités monde),
/// indépendante de l'échelle d'affichage.
#[derive(Component)]
struct Particle {
    pos: Vec2,
}

/// Marqueur + état d'une traînée pour les marcheurs mis en valeur.
#[derive(Component)]
struct Tracer {
    trail: VecDeque<Vec2>,
    color: Color,
}

/// Nœud de texte du HUD.
#[derive(Component)]
struct HudText;

// ---------------------------------------------------------------------------
// Ressources
// ---------------------------------------------------------------------------

#[derive(Resource)]
struct Sim {
    time: f32,
    paused: bool,
    speed: f32,
}

impl Default for Sim {
    fn default() -> Self {
        Self {
            time: 0.0,
            paused: false,
            speed: 1.0,
        }
    }
}

/// État d'affichage : échelle de projection lissée + statistiques courantes.
#[derive(Resource)]
struct View {
    scale: f32,
    rms_measured: f32,
    rms_theory: f32,
}

impl Default for View {
    fn default() -> Self {
        Self {
            scale: SCALE_MAX,
            rms_measured: 0.0,
            rms_theory: 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Application
// ---------------------------------------------------------------------------

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                resolution: (WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32).into(),
                title: "Mouvement brownien 2D".to_string(),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::srgb(0.02, 0.02, 0.05)))
        .init_resource::<Sim>()
        .init_resource::<View>()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                handle_input,
                step_physics,
                update_view,
                sync_transforms,
                draw_envelope,
                draw_trails,
                draw_grid,
                update_hud,
            )
                .chain(),
        )
        .run();
}

// ---------------------------------------------------------------------------
// Mise en place
// ---------------------------------------------------------------------------

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.spawn(Camera2d);

    // Maillages partagés (un cercle pour le nuage, un plus gros pour les traceurs).
    let dot_mesh = meshes.add(Circle::new(DOT_RADIUS));
    let tracer_mesh = meshes.add(Circle::new(TRACER_RADIUS));

    // Palette arc-en-ciel partagée : on réutilise PALETTE_SIZE matériaux au lieu
    // d'en créer un par particule.
    let palette: Vec<Handle<ColorMaterial>> = (0..PALETTE_SIZE)
        .map(|k| {
            let hue = 360.0 * k as f32 / PALETTE_SIZE as f32;
            materials.add(ColorMaterial::from(Color::hsla(hue, 0.85, 0.6, 0.9)))
        })
        .collect();

    // Nuage : tous les marcheurs partent de l'origine, teinte selon leur indice
    // (on obtient un anneau de couleurs qui se mélange à mesure de la diffusion).
    for i in 0..N_PARTICLES {
        let mat = palette[i * PALETTE_SIZE / N_PARTICLES].clone();
        commands.spawn((
            Particle { pos: Vec2::ZERO },
            Mesh2d(dot_mesh.clone()),
            MeshMaterial2d(mat),
            Transform::from_xyz(0.0, 0.0, 10.0),
        ));
    }

    // Traceurs : quelques marcheurs brillants laissant une traînée.
    for j in 0..N_TRACERS {
        let hue = 360.0 * j as f32 / N_TRACERS as f32;
        let color = Color::hsl(hue, 0.9, 0.65);
        commands.spawn((
            Particle { pos: Vec2::ZERO },
            Tracer {
                trail: VecDeque::from([Vec2::ZERO]),
                color,
            },
            Mesh2d(tracer_mesh.clone()),
            MeshMaterial2d(materials.add(ColorMaterial::from(Color::srgb(1.0, 1.0, 1.0)))),
            Transform::from_xyz(0.0, 0.0, 20.0),
        ));
    }

    // HUD à l'écran (ASCII : la police par défaut de Bevy ne couvre pas √ ni les
    // diacritiques).
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: 15.0,
            ..default()
        },
        TextColor(Color::srgb(0.9, 0.92, 0.98)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            left: Val::Px(12.0),
            ..default()
        },
        HudText,
    ));
}

// ---------------------------------------------------------------------------
// Entrées clavier
// ---------------------------------------------------------------------------

fn handle_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut sim: ResMut<Sim>,
    mut view: ResMut<View>,
    mut particles: Query<(&mut Particle, Option<&mut Tracer>)>,
) {
    if keyboard.just_pressed(KeyCode::Space) {
        sim.paused = !sim.paused;
    }
    if keyboard.pressed(KeyCode::ArrowUp) {
        sim.speed = (sim.speed + 0.04).min(6.0);
    }
    if keyboard.pressed(KeyCode::ArrowDown) {
        sim.speed = (sim.speed - 0.04).max(0.05);
    }

    // Reset complet : temps, positions et traînées repartent de l'origine.
    if keyboard.just_pressed(KeyCode::KeyR) {
        sim.time = 0.0;
        view.scale = SCALE_MAX;
        for (mut particle, tracer) in particles.iter_mut() {
            particle.pos = Vec2::ZERO;
            if let Some(mut tracer) = tracer {
                tracer.trail.clear();
                tracer.trail.push_back(Vec2::ZERO);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Physique : pas d'Euler-Maruyama pour chaque marcheur
// ---------------------------------------------------------------------------

fn step_physics(
    mut sim: ResMut<Sim>,
    time: Res<Time>,
    mut particles: Query<(&mut Particle, Option<&mut Tracer>)>,
) {
    if sim.paused {
        return;
    }

    // Pas de temps réel (indépendant du framerate), borné pour éviter les sauts
    // après un gel de la fenêtre.
    let dt = (time.delta_secs() * sim.speed).min(0.1);
    if dt <= 0.0 {
        return;
    }
    sim.time += dt;

    // Incrément gaussien : variance 2·D·dt par axe => MSD = 4·D·t en 2D.
    let sigma = (2.0 * DIFFUSION * dt).sqrt();
    let normal = Normal::new(0.0, 1.0).unwrap();
    let mut rng = rand::thread_rng();

    for (mut particle, tracer) in particles.iter_mut() {
        let step = Vec2::new(normal.sample(&mut rng), normal.sample(&mut rng)) * sigma;
        particle.pos += step;

        if let Some(mut tracer) = tracer {
            let pos = particle.pos;
            tracer.trail.push_back(pos);
            if tracer.trail.len() > MAX_TRAIL {
                tracer.trail.pop_front();
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Auto-zoom + statistiques
// ---------------------------------------------------------------------------

fn update_view(sim: Res<Sim>, mut view: ResMut<View>, particles: Query<&Particle>) {
    // RMS mesuré : sqrt(moyenne de |pos|²).
    let mut sum_sq = 0.0;
    let mut count = 0u32;
    for particle in particles.iter() {
        sum_sq += particle.pos.length_squared();
        count += 1;
    }
    view.rms_measured = if count > 0 {
        (sum_sq / count as f32).sqrt()
    } else {
        0.0
    };

    // RMS théorique en 2D : sqrt(4·D·t).
    view.rms_theory = (4.0 * DIFFUSION * sim.time).sqrt();

    // Échelle cible : on veut que 3·RMS occupe VIEW_FILL de la demi-hauteur.
    let half = WINDOW_HEIGHT * 0.5 * VIEW_FILL;
    let extent = view.rms_measured.max(view.rms_theory).max(1.0) * 3.0;
    let target = (half / extent).clamp(SCALE_MIN, SCALE_MAX);

    // Lissage exponentiel pour un zoom fluide.
    view.scale += (target - view.scale) * 0.06;
}

/// Projette les positions de simulation à l'écran selon l'échelle courante.
fn sync_transforms(view: Res<View>, mut particles: Query<(&Particle, &mut Transform)>) {
    for (particle, mut transform) in particles.iter_mut() {
        let screen = particle.pos * view.scale;
        transform.translation.x = screen.x;
        transform.translation.y = screen.y;
    }
}

// ---------------------------------------------------------------------------
// Rendu gizmos : enveloppe, traînées, grille
// ---------------------------------------------------------------------------

/// Cercles d'enveloppe théorique à 1·, 2· et 3·RMS.
fn draw_envelope(mut gizmos: Gizmos, view: Res<View>) {
    let rings = [
        (1.0, Color::srgba(0.35, 0.7, 1.0, 0.55)),
        (2.0, Color::srgba(0.35, 0.7, 1.0, 0.28)),
        (3.0, Color::srgba(0.35, 0.7, 1.0, 0.15)),
    ];
    for (mult, color) in rings {
        let r = view.rms_theory * view.scale * mult;
        if r > 1.0 {
            gizmos
                .circle_2d(Isometry2d::from_translation(Vec2::ZERO), r, color)
                .resolution(96);
        }
    }
}

/// Traînées lumineuses des traceurs, avec dégradé d'opacité vers la queue.
fn draw_trails(mut gizmos: Gizmos, view: Res<View>, tracers: Query<&Tracer>) {
    for tracer in tracers.iter() {
        let pts: Vec<Vec2> = tracer.trail.iter().map(|p| *p * view.scale).collect();
        let len = pts.len();
        if len < 2 {
            continue;
        }
        let base = tracer.color.to_linear();
        for (i, w) in pts.windows(2).enumerate() {
            // Récent = opaque, ancien = transparent.
            let alpha = ((i + 1) as f32 / len as f32).powf(1.5);
            let color = Color::srgba(base.red, base.green, base.blue, alpha);
            gizmos.line_2d(w[0], w[1], color);
        }
    }
}

/// Grille de référence fixe (espace écran) + axes, pour percevoir le mouvement.
fn draw_grid(mut gizmos: Gizmos) {
    let step = 64.0;
    let hw = WINDOW_WIDTH * 0.5;
    let hh = WINDOW_HEIGHT * 0.5;
    let minor = Color::srgba(0.5, 0.5, 0.6, 0.05);

    let mut x = -((hw / step).floor() * step);
    while x <= hw {
        gizmos.line_2d(Vec2::new(x, -hh), Vec2::new(x, hh), minor);
        x += step;
    }
    let mut y = -((hh / step).floor() * step);
    while y <= hh {
        gizmos.line_2d(Vec2::new(-hw, y), Vec2::new(hw, y), minor);
        y += step;
    }

    // Axes principaux.
    gizmos.line_2d(
        Vec2::new(-hw, 0.0),
        Vec2::new(hw, 0.0),
        Color::srgba(1.0, 0.5, 0.5, 0.18),
    );
    gizmos.line_2d(
        Vec2::new(0.0, -hh),
        Vec2::new(0.0, hh),
        Color::srgba(0.5, 0.6, 1.0, 0.18),
    );
}

// ---------------------------------------------------------------------------
// HUD
// ---------------------------------------------------------------------------

fn update_hud(
    sim: Res<Sim>,
    view: Res<View>,
    mut text_q: Query<&mut Text, With<HudText>>,
) {
    let Ok(mut text) = text_q.single_mut() else {
        return;
    };
    let ratio = if view.rms_theory > 1.0 {
        view.rms_measured / view.rms_theory
    } else {
        1.0
    };
    let status = if sim.paused { "  [PAUSE]" } else { "" };

    **text = format!(
        "Mouvement brownien 2D - {N_PARTICLES} marcheurs{status}\n\
         t = {:.2} s    zoom x{:.3}\n\
         RMS mesure = {:.1}    theorie 2*sqrt(D*t) = {:.1}    ratio = {:.3}\n\
         Espace: pause    R: reset    Haut/Bas: vitesse (x{:.2})",
        sim.time, view.scale, view.rms_measured, view.rms_theory, ratio, sim.speed,
    );
}
