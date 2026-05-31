use bevy::prelude::*;
use bevy_rapier3d::prelude::*;

use crate::components::Body;
use crate::config::SOFTENING;
use crate::resources::{Energy, SimSettings};

pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (apply_gravity, compute_energy).chain());
    }
}

/// Calcule et applique les forces gravitationnelles N-body chaque frame.
fn apply_gravity(
    mut query: Query<(Entity, &Body, &Transform, &mut ExternalForce)>,
    settings: Res<SimSettings>,
) {
    if settings.paused {
        return;
    }

    // Collecte les positions/masses pour éviter le double emprunt.
    let snapshot: Vec<(Entity, f32, Vec3)> = query
        .iter()
        .map(|(e, b, t, _)| (e, b.mass, t.translation))
        .collect();

    for (entity, body, _, mut ext) in &mut query {
        let pos = snapshot.iter().find(|(e, _, _)| *e == entity).unwrap().2;
        let mut force = Vec3::ZERO;
        for &(other, other_mass, other_pos) in &snapshot {
            if other == entity {
                continue;
            }
            let r = other_pos - pos;
            let dist2 = r.length_squared() + SOFTENING * SOFTENING;
            let dist = dist2.sqrt();
            // F = G*m_i*m_j / (dist² + ε²)^(3/2) * r̂ · dist = G*m_i*m_j*r / dist³
            force += r * (settings.g * body.mass * other_mass / (dist2 * dist));
        }
        ext.force = force;
    }
}

fn compute_energy(
    query: Query<(&Body, &Transform, &Velocity)>,
    settings: Res<SimSettings>,
    mut energy: ResMut<Energy>,
) {
    let data: Vec<(f32, Vec3, Vec3)> = query
        .iter()
        .map(|(b, t, v)| (b.mass, t.translation, v.linear))
        .collect();

    energy.ke = data
        .iter()
        .map(|(m, _, v)| 0.5 * m * v.length_squared())
        .sum();

    let mut pe = 0.0_f32;
    for i in 0..data.len() {
        for j in (i + 1)..data.len() {
            let r = (data[j].1 - data[i].1).length().max(SOFTENING);
            pe -= settings.g * data[i].0 * data[j].0 / r;
        }
    }
    energy.pe = pe;
}
