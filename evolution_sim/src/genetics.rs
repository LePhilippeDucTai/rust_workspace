//! Génétique : génération de génomes initiaux et opérateurs de reproduction.
//!
//! Le modèle est asexué pour rester simple : un parent suffit. Chaque trait
//! enfant est tiré du trait parent et peut subir une mutation gaussienne
//! avec probabilité `MUTATION_RATE` et amplitude relative `MUTATION_SCALE`.
//! Toutes les valeurs sont clampées aux bornes du trait pour éviter dérive
//! incontrôlée.

use rand::Rng;
use rand_distr::{Distribution, Normal};

use crate::components::Genome;
use crate::config::{
    MUTATION_RATE, MUTATION_SCALE, SIZE_INIT_MEAN, SIZE_INIT_STD, SIZE_MAX, SIZE_MIN,
    SPEED_INIT_MEAN, SPEED_INIT_STD, SPEED_MAX, SPEED_MIN, VISION_INIT_MEAN, VISION_INIT_STD,
    VISION_MAX, VISION_MIN,
};

/// Tire une valeur d'une normale tronquée (rejection sampling court — la queue
/// hors bornes est rare ici donc on accepte un fallback par clamp).
fn sample_truncated<R: Rng>(rng: &mut R, mean: f32, std: f32, lo: f32, hi: f32) -> f32 {
    let normal = Normal::new(mean, std).expect("std > 0");
    // Quelques essais avant fallback clamp pour rester déterministe en perf.
    for _ in 0..8 {
        let v = normal.sample(rng) as f32;
        if v >= lo && v <= hi {
            return v;
        }
    }
    normal.sample(rng).clamp(lo as f64, hi as f64) as f32
}

/// Génère un génome de fondateur (population initiale) selon les distributions
/// configurées dans `config.rs`.
pub fn random_founder_genome<R: Rng>(rng: &mut R) -> Genome {
    Genome {
        speed: sample_truncated(rng, SPEED_INIT_MEAN, SPEED_INIT_STD, SPEED_MIN, SPEED_MAX),
        size: sample_truncated(rng, SIZE_INIT_MEAN, SIZE_INIT_STD, SIZE_MIN, SIZE_MAX),
        vision_range: sample_truncated(rng, VISION_INIT_MEAN, VISION_INIT_STD, VISION_MIN, VISION_MAX),
    }
}

/// Mutation gaussienne sur un trait : écart-type = `MUTATION_SCALE * |valeur|`.
fn maybe_mutate<R: Rng>(rng: &mut R, value: f32, lo: f32, hi: f32) -> f32 {
    if rng.gen::<f32>() >= MUTATION_RATE {
        return value;
    }
    let std = (value.abs() * MUTATION_SCALE).max(1e-3);
    let delta = Normal::new(0.0, std).expect("std > 0").sample(rng) as f32;
    (value + delta).clamp(lo, hi)
}

/// Produit un génome enfant à partir d'un génome parent (reproduction asexuée).
pub fn mutate_genome<R: Rng>(rng: &mut R, parent: &Genome) -> Genome {
    Genome {
        speed: maybe_mutate(rng, parent.speed, SPEED_MIN, SPEED_MAX),
        size: maybe_mutate(rng, parent.size, SIZE_MIN, SIZE_MAX),
        vision_range: maybe_mutate(rng, parent.vision_range, VISION_MIN, VISION_MAX),
    }
}
