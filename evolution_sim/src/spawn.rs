//! Création initiale du monde : caméra, fond, organismes fondateurs, nourriture.
//!
//! Les meshes/matériaux partagés (cercle unitaire, carré nourriture) sont
//! stockés dans la ressource `RenderAssets` pour éviter d'en créer un par
//! entité.

use bevy::prelude::*;
use rand::Rng;

use crate::components::{Energy, Food, Genome, Organism, Velocity};
use crate::config::{
    ENERGY_INIT, FOOD_HALF_SIZE, INITIAL_FOOD, INITIAL_POPULATION, WORLD_HALF_HEIGHT,
    WORLD_HALF_WIDTH, Z_FOOD, Z_ORGANISM,
};
use crate::genetics::random_founder_genome;
use crate::resources::RenderAssets;

pub struct SpawnPlugin;

impl Plugin for SpawnPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_world);
    }
}

fn setup_world(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.spawn(Camera2d);

    let assets = RenderAssets {
        unit_circle: meshes.add(Circle::new(1.0)),
        food_mesh: meshes.add(Rectangle::new(FOOD_HALF_SIZE * 2.0, FOOD_HALF_SIZE * 2.0)),
        food_material: materials.add(Color::srgb(0.35, 0.85, 0.35)),
    };

    let mut rng = rand::thread_rng();
    for _ in 0..INITIAL_POPULATION {
        let genome = random_founder_genome(&mut rng);
        let pos = random_world_position(&mut rng);
        let velocity = random_velocity(&mut rng, genome.speed * 0.5);
        spawn_organism(
            &mut commands,
            &mut materials,
            &assets,
            OrganismSpawn {
                position: pos,
                genome,
                organism: Organism::founder(),
                energy: ENERGY_INIT,
                velocity,
            },
        );
    }

    for _ in 0..INITIAL_FOOD {
        let pos = random_world_position(&mut rng);
        spawn_food(&mut commands, &assets, pos);
    }

    commands.insert_resource(assets);
}

/// Paramètres pour `spawn_organism` : on regroupe pour respecter clippy
/// (`too_many_arguments`) et clarifier l'appel.
pub struct OrganismSpawn {
    pub position: Vec2,
    pub genome: Genome,
    pub organism: Organism,
    pub energy: f32,
    pub velocity: Vec2,
}

/// Spawn d'un organisme. Le mesh est un cercle unitaire scalé à `genome.size`
/// pour que la modification de taille (mutation) ne crée pas de nouveau mesh.
pub fn spawn_organism(
    commands: &mut Commands,
    materials: &mut Assets<ColorMaterial>,
    assets: &RenderAssets,
    spawn: OrganismSpawn,
) {
    let OrganismSpawn {
        position,
        genome,
        organism,
        energy,
        velocity,
    } = spawn;
    let color = color_for_genome(&genome, organism.generation);
    let material = materials.add(color);
    commands.spawn((
        Mesh2d(assets.unit_circle.clone()),
        MeshMaterial2d(material),
        Transform::from_xyz(position.x, position.y, Z_ORGANISM).with_scale(Vec3::new(
            genome.size,
            genome.size,
            1.0,
        )),
        organism,
        genome,
        Energy(energy),
        Velocity(velocity),
    ));
}

/// Spawn d'une particule de nourriture (mesh + matériau partagés).
pub fn spawn_food(commands: &mut Commands, assets: &RenderAssets, position: Vec2) {
    commands.spawn((
        Mesh2d(assets.food_mesh.clone()),
        MeshMaterial2d(assets.food_material.clone()),
        Transform::from_xyz(position.x, position.y, Z_FOOD),
        Food,
    ));
}

pub fn random_world_position<R: Rng>(rng: &mut R) -> Vec2 {
    Vec2::new(
        rng.gen_range(-WORLD_HALF_WIDTH..WORLD_HALF_WIDTH),
        rng.gen_range(-WORLD_HALF_HEIGHT..WORLD_HALF_HEIGHT),
    )
}

pub fn random_velocity<R: Rng>(rng: &mut R, speed: f32) -> Vec2 {
    let angle: f32 = rng.gen_range(0.0..std::f32::consts::TAU);
    Vec2::new(angle.cos(), angle.sin()) * speed
}

/// Encode visuellement les traits :
/// - teinte = vitesse (du bleu lent au rouge rapide),
/// - saturation = fraction « adaptation » (hé, vision élevée),
/// - luminosité = légèrement modulée par la génération (les jeunes plus vifs).
pub fn color_for_genome(g: &Genome, generation: u32) -> Color {
    use crate::config::{SPEED_MAX, SPEED_MIN, VISION_MAX, VISION_MIN};
    let speed_norm = ((g.speed - SPEED_MIN) / (SPEED_MAX - SPEED_MIN)).clamp(0.0, 1.0);
    let vision_norm = ((g.vision_range - VISION_MIN) / (VISION_MAX - VISION_MIN)).clamp(0.0, 1.0);
    // 230° (bleu froid) → 0° (rouge vif) à mesure que la vitesse grimpe.
    let hue = 230.0 * (1.0 - speed_norm);
    let saturation = 0.45 + 0.55 * vision_norm;
    let lightness = 0.45 + 0.10 * ((generation as f32) * 0.07).sin().abs();
    Color::hsl(hue, saturation, lightness)
}
