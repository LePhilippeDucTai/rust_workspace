use bevy::ecs::message::MessageReader;
use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::prelude::*;
use bevy_rapier3d::prelude::Velocity;

use crate::components::{Body, OrbitCamera, Trail};
use crate::config::TRAIL_MAX;
use crate::resources::SimSettings;

pub struct VisualizationPlugin;

impl Plugin for VisualizationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (update_trail, update_body_emissive, draw_trails, orbit_camera),
        );
    }
}

fn update_trail(
    mut query: Query<(&Transform, &mut Trail)>,
    settings: Res<SimSettings>,
) {
    if settings.paused {
        return;
    }
    for (transform, mut trail) in &mut query {
        trail.points.push_back(transform.translation);
        if trail.points.len() > TRAIL_MAX {
            trail.points.pop_front();
        }
    }
}

/// Colore le corps en fonction de sa vitesse : bleu (lent) → rouge (rapide).
fn update_body_emissive(
    query: Query<(&Velocity, &MeshMaterial3d<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    const MAX_SPEED: f32 = 25.0;
    for (vel, mat_handle) in &query {
        let t = (vel.linear.length() / MAX_SPEED).clamp(0.0, 1.0);
        let hue = (1.0 - t) * 220.0; // bleu → rouge
        let color = Color::hsl(hue, 1.0, 0.6);
        if let Some(mat) = materials.get_mut(mat_handle) {
            mat.base_color = color;
            let lin = color.to_linear();
            mat.emissive = LinearRgba::new(lin.red * 4.0, lin.green * 4.0, lin.blue * 4.0, 1.0);
        }
    }
}

fn draw_trails(mut gizmos: Gizmos, query: Query<(&Trail, &Body)>) {
    for (trail, body) in &query {
        let pts: Vec<Vec3> = trail.points.iter().copied().collect();
        let len = pts.len();
        if len < 2 {
            continue;
        }
        let lin = body.base_color.to_linear();
        for (i, w) in pts.windows(2).enumerate() {
            let alpha = ((i + 1) as f32 / len as f32).powi(2);
            let c = Color::srgba(lin.red, lin.green, lin.blue, alpha);
            gizmos.line(w[0], w[1], c);
        }
    }
}

fn orbit_camera(
    mut camera: Query<(&mut Transform, &mut OrbitCamera)>,
    mut evr_motion: MessageReader<MouseMotion>,
    mut evr_scroll: MessageReader<MouseWheel>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
) {
    let Ok((mut t, mut orbit)) = camera.single_mut() else {
        return;
    };

    for ev in evr_scroll.read() {
        orbit.distance = (orbit.distance - ev.y * 3.0).clamp(5.0, 600.0);
    }

    if mouse_buttons.pressed(MouseButton::Left) {
        for ev in evr_motion.read() {
            orbit.yaw -= ev.delta.x * 0.006;
            orbit.pitch = (orbit.pitch - ev.delta.y * 0.006).clamp(-1.45, 1.45);
        }
    }

    let q = Quat::from_euler(EulerRot::YXZ, orbit.yaw, orbit.pitch, 0.0);
    t.translation = orbit.target + q * Vec3::Z * orbit.distance;
    t.look_at(orbit.target, Vec3::Y);
}
