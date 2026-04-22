//! Gestion des entrées utilisateur : clavier (toggles, reset) et souris (spawn).

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::components::Ball;
use crate::config::INITIAL_BALLS;
use crate::resources::SimSettings;
use crate::spawn::spawn_random_ball;

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (handle_toggles, handle_reset, handle_mouse_spawn));
    }
}

fn handle_toggles(keys: Res<ButtonInput<KeyCode>>, mut settings: ResMut<SimSettings>) {
    if keys.just_pressed(KeyCode::KeyG) {
        settings.gravity_enabled = !settings.gravity_enabled;
    }
    if keys.just_pressed(KeyCode::KeyA) {
        settings.collisions_enabled = !settings.collisions_enabled;
    }
    if keys.just_pressed(KeyCode::Space) {
        settings.paused = !settings.paused;
    }
}

fn handle_reset(
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    balls: Query<Entity, With<Ball>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    if !keys.just_pressed(KeyCode::KeyR) {
        return;
    }
    for e in &balls {
        commands.entity(e).despawn();
    }
    let mut rng = rand::thread_rng();
    for _ in 0..INITIAL_BALLS {
        spawn_random_ball(&mut commands, &mut meshes, &mut materials, &mut rng, None);
    }
}

fn handle_mouse_spawn(
    buttons: Res<ButtonInput<MouseButton>>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform)>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    time: Res<Time>,
    mut cooldown: Local<f32>,
) {
    if !buttons.pressed(MouseButton::Left) {
        return;
    }
    *cooldown -= time.delta_secs();
    if *cooldown > 0.0 {
        return;
    }
    *cooldown = 0.05;
    let Ok(window) = window_q.single() else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let Ok((camera, cam_tf)) = camera_q.single() else {
        return;
    };
    let Ok(world) = camera.viewport_to_world_2d(cam_tf, cursor) else {
        return;
    };
    let mut rng = rand::thread_rng();
    spawn_random_ball(
        &mut commands,
        &mut meshes,
        &mut materials,
        &mut rng,
        Some(world),
    );
}
