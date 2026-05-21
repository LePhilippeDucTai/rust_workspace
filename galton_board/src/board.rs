//! Construction et reconstruction de la planche : piquets, murs, cloisons.

use bevy::prelude::*;

use crate::components::{BoardEntity, Divider, Particle, Peg, Wall};
use crate::config::{DIVIDER_HALF_WIDTH, HALF_HEIGHT, PEG_RADIUS};
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
    state.bin_counts = vec![0; config.num_bins()];
    state.spawned_count = 0;
    state.spawn_accumulator = 0.0;
    state.board_dirty = false;
    build_board(&mut commands, &mut meshes, &mut materials, &config, &dims);
}

fn build_board(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    cfg: &BoardConfig,
    dims: &BoardDims,
) {
    spawn_pegs(commands, meshes, materials, cfg, dims);
    spawn_walls(commands, meshes, materials, cfg, dims);
    spawn_dividers(commands, meshes, materials, cfg, dims);
}

fn spawn_pegs(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    cfg: &BoardConfig,
    dims: &BoardDims,
) {
    let peg_mesh = meshes.add(Circle::new(PEG_RADIUS));
    let peg_color = materials.add(Color::srgb(0.70, 0.85, 0.95));
    let s_y = cfg.peg_spacing_y();
    for row in 0..cfg.rows {
        let y = dims.peg_top_y - row as f32 * s_y;
        for col in 0..=row {
            let x = cfg.peg_x(row, col);
            commands.spawn((
                Mesh2d(peg_mesh.clone()),
                MeshMaterial2d(peg_color.clone()),
                Transform::from_xyz(x, y, 1.0),
                Peg { radius: PEG_RADIUS },
                BoardEntity,
            ));
        }
    }
}

fn spawn_walls(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    _cfg: &BoardConfig,
    dims: &BoardDims,
) {
    let wall_color = materials.add(Color::srgb(0.35, 0.40, 0.55));
    let wall_thickness = 6.0;

    // Mur gauche : du haut de la fenêtre jusqu'au sol.
    let wall_height = HALF_HEIGHT * 2.0 - 4.0;
    spawn_wall(
        commands,
        meshes,
        &wall_color,
        Vec2::new(dims.left_wall_x - wall_thickness * 0.5, 0.0),
        Vec2::new(wall_thickness * 0.5, wall_height * 0.5),
        crate::config::WALL_RESTITUTION,
    );
    spawn_wall(
        commands,
        meshes,
        &wall_color,
        Vec2::new(dims.right_wall_x + wall_thickness * 0.5, 0.0),
        Vec2::new(wall_thickness * 0.5, wall_height * 0.5),
        crate::config::WALL_RESTITUTION,
    );

    // Sol.
    let floor_y = dims.bin_bottom_y - wall_thickness * 0.5;
    let floor_color = materials.add(Color::srgb(0.45, 0.45, 0.55));
    spawn_wall(
        commands,
        meshes,
        &floor_color,
        Vec2::new(0.0, floor_y),
        Vec2::new(crate::config::BOARD_WIDTH * 0.5 + wall_thickness, wall_thickness * 0.5),
        crate::config::FLOOR_RESTITUTION,
    );
}

fn spawn_wall(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    color_handle: &Handle<ColorMaterial>,
    center: Vec2,
    half_extents: Vec2,
    restitution: f32,
) {
    let mesh = meshes.add(Rectangle::new(half_extents.x * 2.0, half_extents.y * 2.0));
    commands.spawn((
        Mesh2d(mesh),
        MeshMaterial2d(color_handle.clone()),
        Transform::from_xyz(center.x, center.y, 0.5),
        Wall {
            center,
            half_extents,
            restitution,
        },
        BoardEntity,
    ));
}

/// Cloisons entre bacs : rangée_finale a `rows` piquets, donc `rows-1`
/// cloisons internes ; plus 2 cloisons « bords » au-dessus des murs.
/// Total : rows+1 bacs ↔ rows cloisons internes (entre bacs successifs).
fn spawn_dividers(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    cfg: &BoardConfig,
    dims: &BoardDims,
) {
    let color = materials.add(Color::srgb(0.55, 0.55, 0.70));
    let y_min = dims.bin_bottom_y;
    let y_max = dims.bin_top_y;
    let center_y = (y_min + y_max) * 0.5;
    let half_h = (y_max - y_min) * 0.5;
    let mesh = meshes.add(Rectangle::new(DIVIDER_HALF_WIDTH * 2.0, half_h * 2.0));

    // Cloisons internes : entre bacs i et i+1 (i = 0..rows-1), à
    // x = bin_center(i) + bin_width/2.
    let s = cfg.peg_spacing_x();
    for i in 0..cfg.rows {
        let x = cfg.bin_center_x(i) + s * 0.5;
        commands.spawn((
            Mesh2d(mesh.clone()),
            MeshMaterial2d(color.clone()),
            Transform::from_xyz(x, center_y, 0.5),
            Divider {
                x,
                y_min,
                y_max,
            },
            BoardEntity,
        ));
    }
}

