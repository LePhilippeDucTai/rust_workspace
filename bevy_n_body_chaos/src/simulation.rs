use bevy::prelude::*;
use bevy_rapier3d::prelude::*;

use crate::components::{Body, OrbitCamera, Trail};
use crate::config::BODY_RADIUS;
use crate::resources::SimSettings;
use crate::scenarios::{all_scenarios, BodyInit};

pub struct SimulationPlugin;

impl Plugin for SimulationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_initial_scenario)
            .add_systems(Update, (handle_input, reset_if_needed).chain());
    }
}

fn spawn_initial_scenario(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut settings: ResMut<SimSettings>,
) {
    let scenarios = all_scenarios();
    let scenario = &scenarios[settings.scenario_idx];
    for init in &scenario.bodies {
        spawn_body(&mut commands, &mut meshes, &mut materials, init);
    }
    settings.reset_requested = false;
    spawn_camera(&mut commands, scenario.camera_distance);
}

fn spawn_body(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    init: &BodyInit,
) {
    let mesh = meshes.add(Sphere::new(BODY_RADIUS * init.mass.cbrt()));
    let lin = init.color.to_linear();
    let mat = materials.add(StandardMaterial {
        base_color: init.color,
        emissive: LinearRgba::new(lin.red * 3.0, lin.green * 3.0, lin.blue * 3.0, 1.0),
        ..default()
    });
    commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(mat),
        Transform::from_translation(init.position),
        Body { mass: init.mass, base_color: init.color },
        Trail::default(),
        RigidBody::Dynamic,
        Collider::ball(BODY_RADIUS * init.mass.cbrt()),
        GravityScale(0.0),
        Damping { linear_damping: 0.0, angular_damping: 0.0 },
        ExternalForce::default(),
        Velocity { linear: init.velocity, angular: Vec3::ZERO },
    ));
}

fn spawn_camera(commands: &mut Commands, distance: f32) {
    let pitch = 0.4_f32;
    let yaw = 0.3_f32;
    let q = Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0);
    let pos = q * Vec3::Z * distance;

    commands.spawn((
        Camera3d::default(),
        Transform::from_translation(pos).looking_at(Vec3::ZERO, Vec3::Y),
        OrbitCamera { yaw, pitch, distance, target: Vec3::ZERO },
    ));

    commands.spawn((
        DirectionalLight {
            illuminance: 8000.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.8, 0.5, 0.0)),
    ));
}

fn handle_input(keys: Res<ButtonInput<KeyCode>>, mut settings: ResMut<SimSettings>) {
    if keys.just_pressed(KeyCode::Space) {
        settings.paused = !settings.paused;
    }
    let scenario_key = [
        (KeyCode::Digit1, 0usize),
        (KeyCode::Digit2, 1),
        (KeyCode::Digit3, 2),
        (KeyCode::Digit4, 3),
    ];
    for (key, idx) in scenario_key {
        if keys.just_pressed(key) {
            settings.scenario_idx = idx;
            settings.reset_requested = true;
        }
    }
    if keys.just_pressed(KeyCode::KeyR) {
        settings.reset_requested = true;
    }
}

fn reset_if_needed(
    mut commands: Commands,
    mut settings: ResMut<SimSettings>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    bodies: Query<Entity, With<Body>>,
    cameras: Query<Entity, With<OrbitCamera>>,
) {
    if !settings.reset_requested {
        return;
    }
    settings.reset_requested = false;

    for e in &bodies {
        commands.entity(e).despawn();
    }
    for e in &cameras {
        commands.entity(e).despawn();
    }

    let scenarios = all_scenarios();
    let scenario = &scenarios[settings.scenario_idx];
    for init in &scenario.bodies {
        spawn_body(&mut commands, &mut meshes, &mut materials, init);
    }
    spawn_camera(&mut commands, scenario.camera_distance);
}
