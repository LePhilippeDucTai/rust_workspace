//! Création des balles (initiales, sur clic, sur reset).

use bevy::prelude::*;
use rand::Rng;

use crate::components::{Ball, ColorCycler, Velocity};
use crate::config::{
    INITIAL_BALLS, INITIAL_SPEED, MAX_RADIUS, MIN_RADIUS, WINDOW_HEIGHT, WINDOW_WIDTH,
};

pub struct SpawnPlugin;

impl Plugin for SpawnPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_scene)
            .add_systems(Update, cycle_ball_color);
    }
}

/// Fait osciller la couleur des balles `ColorCycler` : la saturation
/// passe périodiquement par 0 (gris), tandis que la teinte tourne.
fn cycle_ball_color(
    time: Res<Time>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    query: Query<&MeshMaterial2d<ColorMaterial>, With<ColorCycler>>,
) {
    let t = time.elapsed_secs();
    let hue = (t * 60.0) % 360.0;
    let saturation = (t * std::f32::consts::PI / 2.0).sin().abs();
    let color = Color::hsl(hue, saturation, 0.5);

    for handle in &query {
        if let Some(material) = materials.get_mut(&handle.0) {
            material.color = color;
        }
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

    let radius = (MIN_RADIUS + MAX_RADIUS) * 0.5;
    let angle: f32 = rng.gen_range(0.0..std::f32::consts::TAU);
    commands.spawn((
        Mesh2d(meshes.add(Circle::new(radius))),
        MeshMaterial2d(materials.add(Color::srgb(0.5, 0.5, 0.5))),
        Transform::from_xyz(0.0, 0.0, 0.0),
        Ball { radius, mass: radius },
        Velocity(Vec2::new(angle.cos(), angle.sin()) * INITIAL_SPEED),
        ColorCycler,
    ));
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
