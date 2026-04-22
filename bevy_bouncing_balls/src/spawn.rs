//! Création des balles (initiales, sur clic, sur reset).

use bevy::prelude::*;
use rand::Rng;

use crate::components::{Ball, Velocity};
use crate::config::{
    INITIAL_BALLS, INITIAL_SPEED, MAX_RADIUS, MIN_RADIUS, WINDOW_HEIGHT, WINDOW_WIDTH,
};

pub struct SpawnPlugin;

impl Plugin for SpawnPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_scene);
    }
}

fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.spawn(Camera2d);

    let mut rng = rand::thread_rng();
    for _ in 0..INITIAL_BALLS {
        spawn_random_ball(&mut commands, &mut meshes, &mut materials, &mut rng, None);
    }
}

/// Crée une balle aléatoire (rayon, couleur, direction). Si `spawn_pos` est
/// fourni, la balle est placée à cette position (clampée aux murs).
pub fn spawn_random_ball(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    rng: &mut impl Rng,
    spawn_pos: Option<Vec2>,
) {
    let radius: f32 = rng.gen_range(MIN_RADIUS..=MAX_RADIUS);
    let mass = radius;

    let half_w = WINDOW_WIDTH * 0.5 - radius - 2.0;
    let half_h = WINDOW_HEIGHT * 0.5 - radius - 2.0;
    let pos = spawn_pos
        .map(|p| Vec2::new(p.x.clamp(-half_w, half_w), p.y.clamp(-half_h, half_h)))
        .unwrap_or_else(|| {
            Vec2::new(
                rng.gen_range(-half_w..half_w),
                rng.gen_range(-half_h..half_h),
            )
        });

    let angle: f32 = rng.gen_range(0.0..std::f32::consts::TAU);
    let velocity = Vec2::new(angle.cos(), angle.sin()) * INITIAL_SPEED;
    let color = Color::hsl(rng.gen_range(0.0..360.0), 0.75, 0.6);

    commands.spawn((
        Mesh2d(meshes.add(Circle::new(radius))),
        MeshMaterial2d(materials.add(color)),
        Transform::from_xyz(pos.x, pos.y, 0.0),
        Ball { radius, mass },
        Velocity(velocity),
    ));
}
