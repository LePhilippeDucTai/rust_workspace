use std::time::Duration;

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::config::{
    CELL_SIZE, GRID_HEIGHT, GRID_WIDTH, INITIAL_DENSITY, MAX_SPEED_HZ, MIN_SPEED_HZ, SPEED_FACTOR,
};
use crate::grid::Grid;
use crate::simulation::SimState;

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (handle_keyboard, handle_mouse));
    }
}

fn handle_keyboard(
    keys: Res<ButtonInput<KeyCode>>,
    mut sim: ResMut<SimState>,
    mut grid: ResMut<Grid>,
    mut fixed_time: ResMut<Time<Fixed>>,
) {
    if keys.just_pressed(KeyCode::Space) {
        sim.paused = !sim.paused;
    }
    if keys.just_pressed(KeyCode::KeyC) {
        grid.clear();
        sim.generation = 0;
    }
    if keys.just_pressed(KeyCode::KeyR) {
        grid.randomize(INITIAL_DENSITY);
        sim.generation = 0;
    }
    if keys.just_pressed(KeyCode::ArrowUp) {
        let hz = fixed_time.timestep().as_secs_f64().recip();
        fixed_time.set_timestep(Duration::from_secs_f64(
            1.0 / (hz * SPEED_FACTOR).min(MAX_SPEED_HZ),
        ));
    }
    if keys.just_pressed(KeyCode::ArrowDown) {
        let hz = fixed_time.timestep().as_secs_f64().recip();
        fixed_time.set_timestep(Duration::from_secs_f64(
            1.0 / (hz / SPEED_FACTOR).max(MIN_SPEED_HZ),
        ));
    }
}

/// Paints cells on left-click and drag.
///
/// On the initial press the target state is determined by toggling the clicked
/// cell; subsequent drag events set every visited cell to that same state so
/// that a single stroke consistently draws or erases without flickering.
fn handle_mouse(
    buttons: Res<ButtonInput<MouseButton>>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform)>,
    mut grid: ResMut<Grid>,
    mut paint_state: Local<Option<bool>>,
) {
    if buttons.just_released(MouseButton::Left) {
        *paint_state = None;
        return;
    }
    if !buttons.pressed(MouseButton::Left) {
        return;
    }

    let Ok(window) = window_q.single() else { return };
    let Ok((camera, cam_tf)) = camera_q.single() else { return };
    let Some(cursor) = window.cursor_position() else { return };
    let Ok(world) = camera.viewport_to_world_2d(cam_tf, cursor) else { return };

    let half_w = GRID_WIDTH as f32 * CELL_SIZE / 2.0;
    let half_h = GRID_HEIGHT as f32 * CELL_SIZE / 2.0;
    let gx = ((world.x + half_w) / CELL_SIZE).floor() as i32;
    let gy = ((world.y + half_h) / CELL_SIZE).floor() as i32;

    if !(0..GRID_WIDTH as i32).contains(&gx) || !(0..GRID_HEIGHT as i32).contains(&gy) {
        return;
    }
    let (x, y) = (gx as usize, gy as usize);

    if buttons.just_pressed(MouseButton::Left) {
        let target = !grid.get(x, y);
        *paint_state = Some(target);
        grid.set(x, y, target);
    } else if let Some(target) = *paint_state {
        grid.set(x, y, target);
    }
}
