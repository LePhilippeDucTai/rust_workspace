use bevy::prelude::*;
use rand::Rng;
use rand_distr::Normal;
use std::collections::VecDeque;

const WINDOW_WIDTH: f32 = 1200.0;
const WINDOW_HEIGHT: f32 = 1000.0;
const MAX_HISTORY: usize = 500;
const DT: f32 = 0.016;
const DIFFUSION: f32 = 2.0;

#[derive(Component)]
struct Particle {
    velocity: Vec2,
    history: VecDeque<Vec2>,
}

#[derive(Component)]
struct EnvelopeCircle;

#[derive(Resource)]
struct SimulationState {
    time: f32,
    paused: bool,
    speed: f32,
}

#[derive(Resource)]
struct SimStats {
    current_distance: f32,
    theoretical_radius: f32,
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                resolution: (WINDOW_WIDTH, WINDOW_HEIGHT).into(),
                title: "2D Brownian Motion Animation".to_string(),
                ..default()
            }),
            ..default()
        }))
        .init_resource::<SimulationState>()
        .init_resource::<SimStats>()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                handle_input,
                update_physics,
                update_envelope,
                draw_trajectory,
                draw_envelope,
                draw_grid,
                update_stats,
                draw_hud,
            ),
        )
        .run();
}

impl Default for SimulationState {
    fn default() -> Self {
        Self {
            time: 0.0,
            paused: false,
            speed: 1.0,
        }
    }
}

impl Default for SimStats {
    fn default() -> Self {
        Self {
            current_distance: 0.0,
            theoretical_radius: 0.0,
        }
    }
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d::default());

    let mut particle = Particle {
        velocity: Vec2::ZERO,
        history: VecDeque::new(),
    };
    particle.history.push_back(Vec2::ZERO);

    commands.spawn((particle, Transform::from_translation(Vec3::ZERO)));

    commands.spawn((
        EnvelopeCircle,
        Transform::from_translation(Vec3::new(0.0, 0.0, 0.1)),
    ));
}

fn handle_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<SimulationState>,
) {
    if keyboard.just_pressed(KeyCode::Space) {
        state.paused = !state.paused;
    }
    if keyboard.just_pressed(KeyCode::KeyR) {
        state.time = 0.0;
    }
    if keyboard.pressed(KeyCode::ArrowUp) {
        state.speed = (state.speed + 0.05).min(5.0);
    }
    if keyboard.pressed(KeyCode::ArrowDown) {
        state.speed = (state.speed - 0.05).max(0.1);
    }
}

fn update_physics(
    mut particles: Query<&mut Particle>,
    mut state: ResMut<SimulationState>,
) {
    if state.paused {
        return;
    }

    state.time += DT * state.speed;

    let mut rng = rand::thread_rng();
    let normal = Normal::new(0.0, 1.0).unwrap();

    for mut particle in particles.iter_mut() {
        let dt_scaled = DT * state.speed;
        let dw_x = normal.sample(&mut rng) * (dt_scaled * DIFFUSION).sqrt();
        let dw_y = normal.sample(&mut rng) * (dt_scaled * DIFFUSION).sqrt();

        particle.velocity.x += dw_x;
        particle.velocity.y += dw_y;

        let current_pos = particle.history.back().copied().unwrap_or(Vec2::ZERO);
        let new_pos = current_pos + Vec2::new(dw_x, dw_y);

        particle.history.push_back(new_pos);
        if particle.history.len() > MAX_HISTORY {
            particle.history.pop_front();
        }
    }
}

fn update_envelope(
    mut circles: Query<&mut Transform, With<EnvelopeCircle>>,
    state: Res<SimulationState>,
) {
    for mut transform in circles.iter_mut() {
        let radius = (state.time * DIFFUSION).sqrt();
        transform.scale = Vec3::splat(radius);
    }
}

fn update_stats(
    particles: Query<&Particle>,
    mut stats: ResMut<SimStats>,
    state: Res<SimulationState>,
) {
    for particle in particles.iter() {
        if let Some(&pos) = particle.history.back() {
            stats.current_distance = pos.length();
        }
    }
    stats.theoretical_radius = (state.time * DIFFUSION).sqrt();
}

fn draw_trajectory(
    particles: Query<&Particle>,
    mut gizmos: Gizmos,
) {
    for particle in particles.iter() {
        let history: Vec<_> = particle.history.iter().copied().collect();
        let len = history.len();

        for window in history.windows(2) {
            let [p1, p2] = [window[0], window[1]];
            let idx = history.iter().position(|&p| p == p1).unwrap_or(0);
            let t_norm = idx as f32 / len.max(1) as f32;

            let color = Color::hsl(210.0 + t_norm * 150.0, 0.8, 0.5);

            gizmos.line(
                Vec3::new(p1.x, p1.y, 0.0),
                Vec3::new(p2.x, p2.y, 0.0),
                color,
            );
        }

        if let Some(&pos) = particle.history.back() {
            gizmos.circle(
                Vec3::new(pos.x, pos.y, 0.05),
                Direction3d::Z,
                3.0,
                Color::srgb(1.0, 0.2, 0.2),
            );
        }
    }
}

fn draw_envelope(mut gizmos: Gizmos, state: Res<SimulationState>) {
    let radius = (state.time * DIFFUSION).sqrt();

    gizmos.circle(
        Vec3::new(0.0, 0.0, 0.0),
        Direction3d::Z,
        radius,
        Color::srgba(0.3, 0.6, 0.9, 0.15),
    );

    for i in 0..12 {
        let angle = (i as f32 / 12.0) * std::f32::consts::TAU;
        let pos = Vec2::new(angle.cos(), angle.sin()) * radius;
        gizmos.circle(
            Vec3::new(pos.x, pos.y, 0.02),
            Direction3d::Z,
            1.5,
            Color::srgba(0.3, 0.6, 0.9, 0.3),
        );
    }
}

fn draw_grid(mut gizmos: Gizmos, state: Res<SimulationState>) {
    let bounds = (state.time * DIFFUSION).sqrt().max(100.0) * 1.5;
    let grid_step = 50.0;

    let color_minor = Color::srgba(0.4, 0.4, 0.4, 0.1);
    let color_major = Color::srgba(0.5, 0.5, 0.5, 0.2);

    let mut x = -bounds;
    while x <= bounds {
        let color = if (x / grid_step).abs() % 5.0 < 0.01 {
            color_major
        } else {
            color_minor
        };
        gizmos.line(
            Vec3::new(x, -bounds, -1.0),
            Vec3::new(x, bounds, -1.0),
            color,
        );
        x += grid_step;
    }

    let mut y = -bounds;
    while y <= bounds {
        let color = if (y / grid_step).abs() % 5.0 < 0.01 {
            color_major
        } else {
            color_minor
        };
        gizmos.line(
            Vec3::new(-bounds, y, -1.0),
            Vec3::new(bounds, y, -1.0),
            color,
        );
        y += grid_step;
    }

    gizmos.line(
        Vec3::new(-bounds, 0.0, 0.0),
        Vec3::new(bounds, 0.0, 0.0),
        Color::srgba(1.0, 0.5, 0.5, 0.4),
    );
    gizmos.line(
        Vec3::new(0.0, -bounds, 0.0),
        Vec3::new(0.0, bounds, 0.0),
        Color::srgba(0.5, 0.5, 1.0, 0.4),
    );
}

fn draw_hud(
    mut gizmos: Gizmos,
    state: Res<SimulationState>,
    stats: Res<SimStats>,
) {
    let hud_y = WINDOW_HEIGHT / 2.0 - 20.0;
    let hud_x = -WINDOW_WIDTH / 2.0 + 20.0;

    let time_text = format!("Time: {:.2}", state.time);
    let distance_text = format!("Current distance: {:.2}", stats.current_distance);
    let theory_text = format!("Theoretical √t: {:.2}", stats.theoretical_radius);
    let ratio_text = format!(
        "Ratio (d/√t): {:.3}",
        if stats.theoretical_radius > 0.01 {
            stats.current_distance / stats.theoretical_radius
        } else {
            1.0
        }
    );
    let speed_text = format!("Speed: {:.1}x", state.speed);
    let status_text = if state.paused {
        "[PAUSED] Space to resume, R to reset, ↑/↓ speed"
    } else {
        "Space to pause, R to reset, ↑/↓ speed"
    };

    println!("{}", time_text);
    println!("{}", distance_text);
    println!("{}", theory_text);
    println!("{}", ratio_text);
    println!("{}", speed_text);
    println!("{}", status_text);
}
