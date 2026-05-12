use bevy::prelude::*;

use crate::config::{CELL_SIZE, COLOR_ALIVE, COLOR_DEAD, GRID_HEIGHT, GRID_WIDTH};
use crate::grid::Grid;

/// Tags a sprite with its grid coordinates so colours can be synced each step.
#[derive(Component)]
pub struct CellSprite {
    pub x: usize,
    pub y: usize,
}

pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_cells)
            .add_systems(Update, sync_cell_colors);
    }
}

fn spawn_cells(mut commands: Commands) {
    let offset_x = -(GRID_WIDTH as f32 * CELL_SIZE) / 2.0 + CELL_SIZE / 2.0;
    let offset_y = -(GRID_HEIGHT as f32 * CELL_SIZE) / 2.0 + CELL_SIZE / 2.0;

    for y in 0..GRID_HEIGHT {
        for x in 0..GRID_WIDTH {
            commands.spawn((
                Sprite {
                    color: COLOR_DEAD,
                    custom_size: Some(Vec2::splat(CELL_SIZE - 1.0)),
                    ..default()
                },
                Transform::from_xyz(
                    offset_x + x as f32 * CELL_SIZE,
                    offset_y + y as f32 * CELL_SIZE,
                    0.0,
                ),
                CellSprite { x, y },
            ));
        }
    }
}

/// Repaints all cell sprites whenever the grid state changes.
/// The `is_changed` guard keeps this a no-op on frames with no simulation step.
fn sync_cell_colors(grid: Res<Grid>, mut query: Query<(&CellSprite, &mut Sprite)>) {
    if !grid.is_changed() {
        return;
    }
    for (cell, mut sprite) in &mut query {
        sprite.color = if grid.get(cell.x, cell.y) {
            COLOR_ALIVE
        } else {
            COLOR_DEAD
        };
    }
}
