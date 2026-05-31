//! Comportement des organismes : recherche de nourriture, errance, intégration.
//!
//! Pipeline par frame (FixedUpdate, dans l'ordre `chain()`) :
//!   1. `tick_organism_timers` : avance les cooldowns (reproduction).
//!   2. `decide_direction` : choisit une direction cible (nourriture la plus
//!      proche dans la portée de vision, sinon errance lissée).
//!   3. `integrate_motion` : déplace l'organisme + applique le coût énergétique
//!      proportionnel à v² + métabolisme basal.
//!   4. `eat_food` : capture la nourriture dans le rayon d'organisme.
//!   5. `clamp_to_world` : rebond doux sur les murs.
//!
//! La détection de nourriture est en O(N×M) (N organismes × M nourritures).
//! Pour ~200 organismes × ~300 food, ça reste très en-dessous des 16 ms / frame
//! sur une machine moderne. Si la population explose, on pourrait passer à un
//! quadtree, mais c'est inutile pour la cible (≤ 600).

use bevy::prelude::*;
use rand::Rng;
use rand_distr::{Distribution, Normal};

use crate::components::{Energy, Food, Genome, Organism, Velocity};
use crate::config::{
    BASAL_COST_PER_SEC, ENERGY_MAX, FOOD_ENERGY, MOVEMENT_COST_COEF, SIZE_METABOLIC_COEF,
    STEERING_RATE, VISION_COST_COEF, WANDER_TURN_RATE, WORLD_HALF_HEIGHT, WORLD_HALF_WIDTH,
};
use crate::resources::{SimulationSet, not_paused};

pub struct BehaviorPlugin;

impl Plugin for BehaviorPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            (
                tick_organism_timers,
                decide_direction,
                integrate_motion,
                eat_food,
                clamp_to_world,
            )
                .chain()
                .in_set(SimulationSet::Behavior)
                .run_if(not_paused),
        );
    }
}

fn tick_organism_timers(time: Res<Time<Fixed>>, mut q: Query<&mut Organism>) {
    let dt = time.delta_secs();
    for mut o in &mut q {
        o.time_since_reproduction += dt;
    }
}

/// Choisit la direction cible et lisse la vélocité courante vers elle.
fn decide_direction(
    time: Res<Time<Fixed>>,
    food_q: Query<&Transform, (With<Food>, Without<Organism>)>,
    mut org_q: Query<(&Transform, &Genome, &mut Velocity), With<Organism>>,
) {
    let dt = time.delta_secs();
    let mut rng = rand::thread_rng();
    let normal = Normal::new(0.0_f32, WANDER_TURN_RATE).expect("std > 0");

    // Petite copie pour éviter d'itérer sur la query plusieurs fois (et garder
    // un coût modeste : ~300 Vec2 = 4.8 ko, négligeable).
    let foods: Vec<Vec2> = food_q.iter().map(|t| t.translation.truncate()).collect();

    for (org_tf, genome, mut vel) in &mut org_q {
        let pos = org_tf.translation.truncate();
        let vision_sq = genome.vision_range * genome.vision_range;

        // Recherche linéaire de la nourriture la plus proche dans la vision.
        let mut best: Option<(Vec2, f32)> = None;
        for &f in &foods {
            let d2 = pos.distance_squared(f);
            if d2 <= vision_sq && best.is_none_or(|(_, b)| d2 < b) {
                best = Some((f, d2));
            }
        }

        let desired_dir = if let Some((target, _)) = best {
            (target - pos).normalize_or_zero()
        } else {
            // Errance : on perturbe l'angle courant par un bruit gaussien.
            let current_angle = if vel.0.length_squared() > 1e-6 {
                vel.0.y.atan2(vel.0.x)
            } else {
                rng.gen_range(0.0..std::f32::consts::TAU)
            };
            let new_angle = current_angle + normal.sample(&mut rng) * dt;
            Vec2::new(new_angle.cos(), new_angle.sin())
        };

        let target_vel = desired_dir * genome.speed;
        let alpha = (STEERING_RATE * dt).clamp(0.0, 1.0);
        vel.0 = vel.0.lerp(target_vel, alpha);
    }
}

/// Intègre la position et applique le coût énergétique.
fn integrate_motion(
    time: Res<Time<Fixed>>,
    mut q: Query<(&mut Transform, &Velocity, &Genome, &mut Energy), With<Organism>>,
) {
    let dt = time.delta_secs();
    for (mut tf, vel, genome, mut energy) in &mut q {
        tf.translation.x += vel.0.x * dt;
        tf.translation.y += vel.0.y * dt;

        // Coûts énergétiques :
        //   métabolisme basal + pénalité de taille + mouvement (k*v²) + vision.
        let speed_sq = vel.0.length_squared();
        let cost = BASAL_COST_PER_SEC
            + SIZE_METABOLIC_COEF * genome.size
            + MOVEMENT_COST_COEF * speed_sq
            + VISION_COST_COEF * genome.vision_range;
        energy.0 -= cost * dt;
    }
}

/// Détection de capture nourriture : un organisme mange toute nourriture
/// présente dans son rayon (genome.size + tolérance visuelle).
fn eat_food(
    mut commands: Commands,
    mut org_q: Query<(&Transform, &Genome, &mut Energy), With<Organism>>,
    food_q: Query<(Entity, &Transform), With<Food>>,
) {
    // Snapshot pour O(N×M) avec marquage in-place pour éviter les double-despawn.
    let foods: Vec<(Entity, Vec2)> = food_q
        .iter()
        .map(|(e, t)| (e, t.translation.truncate()))
        .collect();
    let mut eaten = vec![false; foods.len()];

    for (org_tf, genome, mut energy) in &mut org_q {
        let pos = org_tf.translation.truncate();
        let bite_radius = genome.size + 2.0;
        let bite_sq = bite_radius * bite_radius;

        for (i, (_, fpos)) in foods.iter().enumerate() {
            if eaten[i] {
                continue;
            }
            if pos.distance_squared(*fpos) <= bite_sq {
                eaten[i] = true;
                energy.0 = (energy.0 + FOOD_ENERGY).min(ENERGY_MAX);
            }
        }
    }

    for (i, (entity, _)) in foods.iter().enumerate() {
        if eaten[i] {
            commands.entity(*entity).despawn();
        }
    }
}

/// Garde les organismes dans l'aire de jeu (rebond élastique simple).
fn clamp_to_world(mut q: Query<(&mut Transform, &mut Velocity), With<Organism>>) {
    for (mut tf, mut vel) in &mut q {
        if tf.translation.x < -WORLD_HALF_WIDTH {
            tf.translation.x = -WORLD_HALF_WIDTH;
            vel.0.x = vel.0.x.abs();
        } else if tf.translation.x > WORLD_HALF_WIDTH {
            tf.translation.x = WORLD_HALF_WIDTH;
            vel.0.x = -vel.0.x.abs();
        }
        if tf.translation.y < -WORLD_HALF_HEIGHT {
            tf.translation.y = -WORLD_HALF_HEIGHT;
            vel.0.y = vel.0.y.abs();
        } else if tf.translation.y > WORLD_HALF_HEIGHT {
            tf.translation.y = WORLD_HALF_HEIGHT;
            vel.0.y = -vel.0.y.abs();
        }
    }
}
