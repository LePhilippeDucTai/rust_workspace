//! Création de la scène : caméra, bloc de fluide initial, entités de rendu.

use bevy::prelude::*;
use rand::Rng;

use crate::components::{Particle, ParticleId};
use crate::config::{
    BOX_BOTTOM, BOX_LEFT, INIT_COLUMNS, INIT_ROWS, PARTICLE_RADIUS, SPACING,
};
use crate::physics::solver::Fluid;
use crate::resources::{FluidSim, InitialLayout};

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

    // Construit le bloc initial de particules (réseau régulier + léger jitter
    // pour briser la symétrie parfaite et éviter les artefacts en colonnes).
    let mut fluid = Fluid::new();
    let mut rng = rand::thread_rng();
    let jitter = SPACING * 0.1;
    for row in 0..INIT_ROWS {
        for col in 0..INIT_COLUMNS {
            let x = BOX_LEFT + 50.0 + col as f32 * SPACING + rng.gen_range(-jitter..jitter);
            let y = BOX_BOTTOM + 50.0 + row as f32 * SPACING + rng.gen_range(-jitter..jitter);
            fluid.add_particle(Vec2::new(x, y));
        }
    }
    fluid.calibrate_rest_density();

    let initial: Vec<Vec2> = fluid.pos.clone();

    // Une particule = un sprite circulaire. Maillage partagé, matériau propre
    // à chaque entité pour pouvoir la colorer selon sa vitesse.
    let mesh = meshes.add(Circle::new(PARTICLE_RADIUS));
    for i in 0..fluid.len() {
        let p = fluid.pos[i];
        commands.spawn((
            Mesh2d(mesh.clone()),
            MeshMaterial2d(materials.add(Color::srgb(0.2, 0.5, 1.0))),
            Transform::from_xyz(p.x, p.y, 0.0),
            Particle,
            ParticleId(i),
        ));
    }

    commands.insert_resource(InitialLayout(initial));
    commands.insert_resource(FluidSim(fluid));
}
