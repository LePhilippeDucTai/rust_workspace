//! Construction de la planche : piquets, murs, sol et cloisons (tous fixes).
//! Toutes les collisions sont déléguées à Rapier via `RigidBody::Fixed`.

use bevy::prelude::*;
use bevy_rapier2d::prelude::*;

use crate::components::{BoardEntity, Particle, Peg};
use crate::config::{
    BOARD_WIDTH, DIVIDER_HALF_WIDTH, PEG_FRICTION, PEG_RADIUS, PEG_RESTITUTION, WALL_FRICTION,
    WALL_RESTITUTION,
};
use crate::resources::{BoardConfig, BoardDims, SimState};

pub struct BoardPlugin;

impl Plugin for BoardPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(BoardDims::default())
            .add_systems(Startup, setup_camera_and_board)
            .add_systems(Update, rebuild_on_dirty);
    }
}

fn setup_camera_and_board(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    config: Res<BoardConfig>,
    mut dims: ResMut<BoardDims>,
    mut state: ResMut<SimState>,
) {
    commands.spawn(Camera2d);
    *dims = BoardDims::from_config(&config);
    state.bin_counts = vec![0; config.num_bins()];
    build_board(&mut commands, &mut meshes, &mut materials, &config, &dims);
}

#[allow(clippy::too_many_arguments)]
fn rebuild_on_dirty(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    config: Res<BoardConfig>,
    mut dims: ResMut<BoardDims>,
    mut state: ResMut<SimState>,
    board_entities: Query<Entity, With<BoardEntity>>,
    particles: Query<Entity, With<Particle>>,
) {
    if !state.board_dirty {
        return;
    }
    for e in &board_entities {
        commands.entity(e).despawn();
    }
    for e in &particles {
        commands.entity(e).despawn();
    }
    *dims = BoardDims::from_config(&config);
    reset_sim_state(&mut state, config.num_bins());
    build_board(&mut commands, &mut meshes, &mut materials, &config, &dims);
}

fn reset_sim_state(state: &mut SimState, num_bins: usize) {
    state.bin_counts = vec![0; num_bins];
    state.spawned_count = 0;
    state.spawn_accumulator = 0.0;
    state.board_dirty = false;
}

fn build_board(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    cfg: &BoardConfig,
    dims: &BoardDims,
) {
    spawn_funnel(commands, meshes, materials, cfg, dims);
    spawn_pegs(commands, meshes, materials, cfg, dims);
    spawn_walls(commands, meshes, materials, dims);
    spawn_dividers(commands, meshes, materials, cfg, dims);
}

/// Entonnoir : deux murs inclinés continus (un gauche, un droit) formés d'un
/// seul cuboid rotatif chacun.  Le résultat est une vraie paroi inclinée sans
/// escaliers ni discontinuités.
fn spawn_funnel(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    cfg: &BoardConfig,
    dims: &BoardDims,
) {
    let color = materials.add(Color::srgb(0.25, 0.33, 0.60));
    // Haut de l'entonnoir : exactement la demi-largeur de la planche, flush
    // avec les murs latéraux — aucun gap sur les côtés.
    let start_half_gap = BOARD_WIDTH * 0.5;
    // Goulot légèrement agrandi : ~2.5 diamètres de particule pour éviter
    // les blocages sans laisser passer plusieurs balles de front.
    let end_half_gap = cfg.particle_radius * 2.5 + 2.0;
    // Limites verticales : le mur démarre au point de spawn (plus haut),
    // et se referme jusqu'à mi-chemin entre peg_top_y et la rangée suivante.
    let y_top = dims.spawn_y;
    let y_bot = dims.peg_top_y + cfg.peg_spacing_y() * 4.0;

    spawn_funnel_wall(
        commands, meshes, &color,
        start_half_gap, y_top,
        end_half_gap,   y_bot,
        5.0,
    );
    spawn_funnel_wall(
        commands, meshes, &color,
        -start_half_gap, y_top,
        -end_half_gap,   y_bot,
        5.0,
    );
}

/// Crée un mur incliné entre (x1, y1) et (x2, y2) avec l'épaisseur donnée.
#[allow(clippy::too_many_arguments)]
fn spawn_funnel_wall(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    color: &Handle<ColorMaterial>,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    thickness: f32,
) {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let length = (dx * dx + dy * dy).sqrt();
    if length < 1.0 {
        return;
    }
    let angle = dy.atan2(dx); // angle de l'axe long du rectangle
    let cx = (x1 + x2) * 0.5;
    let cy = (y1 + y2) * 0.5;
    let mesh = meshes.add(Rectangle::new(length, thickness));
    commands.spawn((
        Mesh2d(mesh),
        MeshMaterial2d(color.clone()),
        Transform::from_translation(Vec3::new(cx, cy, 0.5))
            .with_rotation(Quat::from_rotation_z(angle)),
        RigidBody::Fixed,
        Collider::cuboid(length * 0.5, thickness * 0.5),
        Restitution::coefficient(WALL_RESTITUTION),
        Friction::coefficient(WALL_FRICTION),
        BoardEntity,
    ));
}

fn spawn_pegs(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    cfg: &BoardConfig,
    dims: &BoardDims,
) {
    let mesh = meshes.add(Circle::new(PEG_RADIUS));
    let color = materials.add(Color::srgb(0.88, 0.95, 1.00));
    let s_y = cfg.peg_spacing_y();
    for row in 0..cfg.rows {
        let y = dims.peg_top_y - row as f32 * s_y;
        for col in 0..=row {
            let x = cfg.peg_x(row, col);
            commands.spawn((
                Mesh2d(mesh.clone()),
                MeshMaterial2d(color.clone()),
                Transform::from_xyz(x, y, 1.0),
                RigidBody::Fixed,
                Collider::ball(PEG_RADIUS),
                Restitution::coefficient(PEG_RESTITUTION),
                Friction::coefficient(PEG_FRICTION),
                Peg,
                BoardEntity,
            ));
        }
    }
}

fn spawn_walls(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    dims: &BoardDims,
) {
    let wc = materials.add(Color::srgb(0.20, 0.28, 0.55));
    let t = 6.0_f32;

    // Murs latéraux rectangulaires : de la zone des piquets jusqu'au sol.
    let wall_center_y = (dims.peg_top_y + dims.bin_bottom_y) * 0.5;
    let wall_half_h = (dims.peg_top_y - dims.bin_bottom_y) * 0.5;
    spawn_static_box(
        commands,
        meshes,
        &wc,
        Vec2::new(dims.left_wall_x - t * 0.5, wall_center_y),
        Vec2::new(t * 0.5, wall_half_h),
    );
    spawn_static_box(
        commands,
        meshes,
        &wc,
        Vec2::new(dims.right_wall_x + t * 0.5, wall_center_y),
        Vec2::new(t * 0.5, wall_half_h),
    );

    // Sol (couvre la zone des bacs).
    let fc = materials.add(Color::srgb(0.22, 0.30, 0.58));
    spawn_static_box(
        commands,
        meshes,
        &fc,
        Vec2::new(0.0, dims.bin_bottom_y - t * 0.5),
        Vec2::new(BOARD_WIDTH * 0.5 + t, t * 0.5),
    );
}

fn spawn_dividers(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    cfg: &BoardConfig,
    dims: &BoardDims,
) {
    let color = materials.add(Color::srgb(0.30, 0.38, 0.72));
    let (y_min, y_max) = (dims.bin_bottom_y, dims.bin_top_y);
    let (center_y, half_h) = ((y_min + y_max) * 0.5, (y_max - y_min) * 0.5);
    let s = cfg.peg_spacing_x();
    for i in 0..cfg.rows {
        let x = cfg.bin_center_x(i) + s * 0.5;
        spawn_static_box(
            commands,
            meshes,
            &color,
            Vec2::new(x, center_y),
            Vec2::new(DIVIDER_HALF_WIDTH, half_h),
        );
    }
}

fn spawn_static_box(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    color: &Handle<ColorMaterial>,
    center: Vec2,
    half_extents: Vec2,
) {
    let mesh = meshes.add(Rectangle::new(half_extents.x * 2.0, half_extents.y * 2.0));
    commands.spawn((
        Mesh2d(mesh),
        MeshMaterial2d(color.clone()),
        Transform::from_xyz(center.x, center.y, 0.5),
        RigidBody::Fixed,
        Collider::cuboid(half_extents.x, half_extents.y),
        Restitution::coefficient(WALL_RESTITUTION),
        Friction::coefficient(WALL_FRICTION),
        BoardEntity,
    ));
}
