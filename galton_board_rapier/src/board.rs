//! Construction de la planche : piquets, murs, sol et cloisons (tous fixes).
//! Toutes les collisions sont déléguées à Rapier via `RigidBody::Fixed`.

use bevy::prelude::*;
use bevy_rapier2d::prelude::*;

use crate::components::{BoardEntity, Particle, Peg};
use crate::config::{
    BOARD_WIDTH, DIVIDER_HALF_WIDTH, HALF_HEIGHT, PEG_FRICTION, PEG_RADIUS, PEG_RESTITUTION,
    WALL_FRICTION, WALL_RESTITUTION,
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
    spawn_walls(commands, meshes, materials, dims);
    spawn_hopper(commands, meshes, materials, cfg, dims);
    spawn_pegs(commands, meshes, materials, cfg, dims);
    spawn_dividers(commands, meshes, materials, cfg, dims);
}

/// Entonnoir centré en haut de la planche. Les murs diagonaux convergent
/// depuis les bords jusqu'à une ouverture étroite centrée sur le premier piquet.
fn spawn_hopper(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    cfg: &BoardConfig,
    dims: &BoardDims,
) {
    let color = materials.add(Color::srgb(0.40, 0.50, 0.78));
    // Les murs de l'entonnoir partent au-delà des murs latéraux du plateau pour
    // supprimer tout interstice : le récipient est entièrement fermé.
    let half_top = BOARD_WIDTH * 0.5 + 12.0;
    // Col : 3 billes de large pour un débit correct sans bouchon.
    let half_bot = cfg.particle_radius * 3.0;
    // y_top > y_bot (coordonnées Bevy, +Y vers le haut).
    let y_top = HALF_HEIGHT - 10.0;
    // Bas de l'entonnoir : juste au-dessus du premier piquet.
    let y_bot = dims.peg_top_y + cfg.peg_spacing_y() * 0.8;

    spawn_diagonal_wall(commands, meshes, &color, half_top, y_top, half_bot, y_bot, 5.0);
    spawn_diagonal_wall(commands, meshes, &color, -half_top, y_top, -half_bot, y_bot, 5.0);
}

#[allow(clippy::too_many_arguments)]
fn spawn_diagonal_wall(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    color: &Handle<ColorMaterial>,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    thickness: f32,
) {
    let (dx, dy) = (x2 - x1, y2 - y1);
    let length = (dx * dx + dy * dy).sqrt();
    if length < 1.0 {
        return;
    }
    let angle = dy.atan2(dx);
    let mesh = meshes.add(Rectangle::new(length, thickness));
    commands.spawn((
        Mesh2d(mesh),
        MeshMaterial2d(color.clone()),
        Transform::from_translation(Vec3::new((x1 + x2) * 0.5, (y1 + y2) * 0.5, 0.5))
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

    // Murs latéraux rectangulaires : du haut de la fenêtre jusqu'au sol.
    let wall_center_y = (HALF_HEIGHT + dims.bin_bottom_y) * 0.5;
    let wall_half_h = (HALF_HEIGHT - dims.bin_bottom_y) * 0.5;
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
