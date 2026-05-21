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
    spawn_funnel(commands, meshes, materials, cfg, dims);
    spawn_pegs(commands, meshes, materials, cfg, dims);
    spawn_walls(commands, meshes, materials, dims);
    spawn_dividers(commands, meshes, materials, cfg, dims);
}

fn spawn_funnel(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<ColorMaterial>, cfg: &BoardConfig, dims: &BoardDims) {
    let color = materials.add(Color::srgb(0.25, 0.33, 0.60));
    let num_steps = 10usize;
    let start_half_gap = BOARD_WIDTH * 0.40;
    let end_half_gap = cfg.peg_spacing_x();
    let total_h = dims.spawn_y - dims.peg_top_y;
    let step_h = total_h / num_steps as f32;
    let wall_h = step_h * 0.45;
    let half_board = BOARD_WIDTH * 0.5;

    for i in 0..num_steps {
        let t = i as f32 / (num_steps - 1) as f32;
        let half_gap = start_half_gap + (end_half_gap - start_half_gap) * t;
        let y = dims.spawn_y - (i as f32 + 0.5) * step_h;
        let wall_half_w = (half_board - half_gap) * 0.5;
        if wall_half_w <= 0.0 { continue; }
        let left_cx = -(half_gap + wall_half_w);
        let right_cx = half_gap + wall_half_w;
        spawn_static_box(commands, meshes, &color, Vec2::new(left_cx, y), Vec2::new(wall_half_w, wall_h));
        spawn_static_box(commands, meshes, &color, Vec2::new(right_cx, y), Vec2::new(wall_half_w, wall_h));
    }
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
    let (t, h) = (6.0f32, HALF_HEIGHT - 2.0);
    spawn_static_box(
        commands,
        meshes,
        &wc,
        Vec2::new(dims.left_wall_x - t * 0.5, 0.0),
        Vec2::new(t * 0.5, h),
    );
    spawn_static_box(
        commands,
        meshes,
        &wc,
        Vec2::new(dims.right_wall_x + t * 0.5, 0.0),
        Vec2::new(t * 0.5, h),
    );
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
