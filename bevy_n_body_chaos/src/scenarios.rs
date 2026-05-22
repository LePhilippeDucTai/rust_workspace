use bevy::prelude::*;
use rand::Rng;

pub struct BodyInit {
    pub mass: f32,
    pub position: Vec3,
    pub velocity: Vec3,
    pub color: Color,
}

pub struct Scenario {
    pub name: &'static str,
    pub bodies: Vec<BodyInit>,
    pub camera_distance: f32,
}

/// Figure-en-8 de Chenciner-Montgomery (G=1, m=1), mis à l'échelle L=10 pour G_sim=1000.
pub fn figure_eight() -> Scenario {
    let s = 10.0_f32;
    Scenario {
        name: "Figure en 8",
        bodies: vec![
            BodyInit {
                mass: 1.0,
                position: Vec3::new(-0.97000436 * s, 0.24308753 * s, 0.0),
                velocity: Vec3::new(0.46620368 * s, 0.43236573 * s, 0.0),
                color: Color::hsl(200.0, 0.9, 0.65),
            },
            BodyInit {
                mass: 1.0,
                position: Vec3::new(0.97000436 * s, -0.24308753 * s, 0.0),
                velocity: Vec3::new(0.46620368 * s, 0.43236573 * s, 0.0),
                color: Color::hsl(30.0, 0.9, 0.65),
            },
            BodyInit {
                mass: 1.0,
                position: Vec3::ZERO,
                velocity: Vec3::new(-0.93240737 * s, -0.86473146 * s, 0.0),
                color: Color::hsl(120.0, 0.9, 0.65),
            },
        ],
        camera_distance: 35.0,
    }
}

/// Problème pythagoricien : masses 3, 4, 5 aux coins d'un triangle rectangle, vitesses nulles.
/// Extrêmement chaotique, mène à l'éjection d'un corps.
pub fn pythagorean() -> Scenario {
    let s = 6.0_f32;
    Scenario {
        name: "Triangle Pythagorique",
        bodies: vec![
            BodyInit {
                mass: 3.0,
                position: Vec3::new(1.0 * s, 3.0 * s, 0.0),
                velocity: Vec3::ZERO,
                color: Color::hsl(0.0, 0.9, 0.65),
            },
            BodyInit {
                mass: 4.0,
                position: Vec3::new(-2.0 * s, -1.0 * s, 0.0),
                velocity: Vec3::ZERO,
                color: Color::hsl(60.0, 0.9, 0.65),
            },
            BodyInit {
                mass: 5.0,
                position: Vec3::new(1.0 * s, -1.0 * s, 0.0),
                velocity: Vec3::ZERO,
                color: Color::hsl(280.0, 0.9, 0.65),
            },
        ],
        camera_distance: 55.0,
    }
}

/// Triangle équilatéral de Lagrange : solution périodique stable (légèrement perturbée en z).
/// Circumradius R=11.55, ω=0.612 rad/s pour G=1000, m=1, a=20.
pub fn lagrange_triangle() -> Scenario {
    let r = 11.547_f32; // R = a/sqrt(3), a=20
    let v = 7.07_f32; // ω*R = 0.6124 * 11.547
    let dz = 0.8_f32; // léger biais z pour rendre l'orbite 3D visuellement
    Scenario {
        name: "Triangle de Lagrange (3D)",
        bodies: vec![
            BodyInit {
                mass: 1.0,
                position: Vec3::new(0.0, r, dz),
                velocity: Vec3::new(-v, 0.0, 0.0),
                color: Color::hsl(180.0, 0.9, 0.65),
            },
            BodyInit {
                mass: 1.0,
                position: Vec3::new(r * 0.866, -r * 0.5, -dz * 0.5),
                velocity: Vec3::new(v * 0.5, v * 0.866, 0.0),
                color: Color::hsl(300.0, 0.9, 0.65),
            },
            BodyInit {
                mass: 1.0,
                position: Vec3::new(-r * 0.866, -r * 0.5, -dz * 0.5),
                velocity: Vec3::new(v * 0.5, -v * 0.866, 0.0),
                color: Color::hsl(60.0, 0.9, 0.65),
            },
        ],
        camera_distance: 40.0,
    }
}

/// Trois corps 3D aléatoires, impulsion totale nulle (barycentre fixe).
pub fn random_3d() -> Scenario {
    let mut rng = rand::thread_rng();
    let hues = [
        rng.gen_range(0.0_f32..360.0),
        rng.gen_range(0.0..360.0),
        rng.gen_range(0.0..360.0),
    ];
    let masses: Vec<f32> = (0..3).map(|_| rng.gen_range(0.5_f32..3.0)).collect();
    let positions: Vec<Vec3> = (0..3)
        .map(|_| {
            Vec3::new(
                rng.gen_range(-10.0_f32..10.0),
                rng.gen_range(-10.0..10.0),
                rng.gen_range(-5.0..5.0),
            )
        })
        .collect();
    let mut velocities: Vec<Vec3> = (0..3)
        .map(|_| {
            Vec3::new(
                rng.gen_range(-3.0_f32..3.0),
                rng.gen_range(-3.0..3.0),
                rng.gen_range(-1.5..1.5),
            )
        })
        .collect();

    // Annuler l'impulsion totale pour garder le centre de masse fixe.
    let total_mass: f32 = masses.iter().sum();
    let total_p: Vec3 = velocities.iter().zip(&masses).map(|(v, &m)| *v * m).sum();
    let cm_vel = total_p / total_mass;
    for v in &mut velocities {
        *v -= cm_vel;
    }

    Scenario {
        name: "Aléatoire 3D",
        bodies: (0..3)
            .map(|i| BodyInit {
                mass: masses[i],
                position: positions[i],
                velocity: velocities[i],
                color: Color::hsl(hues[i], 0.9, 0.65),
            })
            .collect(),
        camera_distance: 40.0,
    }
}

pub fn all_scenarios() -> Vec<Scenario> {
    vec![
        figure_eight(),
        pythagorean(),
        lagrange_triangle(),
        random_3d(),
    ]
}
