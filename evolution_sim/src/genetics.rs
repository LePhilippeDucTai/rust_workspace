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
        let v: f32 = normal.sample(rng);
        if v >= lo && v <= hi {
            return v;
        }
    }
    let v: f32 = normal.sample(rng);
    v.clamp(lo, hi)
}

/// Génère un génome de fondateur (population initiale) selon les distributions
/// configurées dans `config.rs`.
pub fn random_founder_genome<R: Rng>(rng: &mut R) -> Genome {
    Genome {
        speed: sample_truncated(rng, SPEED_INIT_MEAN, SPEED_INIT_STD, SPEED_MIN, SPEED_MAX),
        size: sample_truncated(rng, SIZE_INIT_MEAN, SIZE_INIT_STD, SIZE_MIN, SIZE_MAX),
        vision_range: sample_truncated(
            rng,
            VISION_INIT_MEAN,
            VISION_INIT_STD,
            VISION_MIN,
            VISION_MAX,
        ),
    }
}

/// Mutation gaussienne sur un trait : écart-type = `MUTATION_SCALE * |valeur|`.
fn maybe_mutate<R: Rng>(rng: &mut R, value: f32, lo: f32, hi: f32) -> f32 {
    let roll: f32 = rng.r#gen();
    if roll >= MUTATION_RATE {
        return value;
    }
    let std = (value.abs() * MUTATION_SCALE).max(1e-3);
    let delta: f32 = Normal::new(0.0_f32, std).expect("std > 0").sample(rng);
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

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    #[test]
    fn founder_genomes_respect_bounds() {
        let mut rng = StdRng::seed_from_u64(42);
        for _ in 0..1000 {
            let g = random_founder_genome(&mut rng);
            assert!(
                g.speed >= SPEED_MIN && g.speed <= SPEED_MAX,
                "speed out of bounds: {}",
                g.speed
            );
            assert!(
                g.size >= SIZE_MIN && g.size <= SIZE_MAX,
                "size out of bounds: {}",
                g.size
            );
            assert!(
                g.vision_range >= VISION_MIN && g.vision_range <= VISION_MAX,
                "vision out of bounds: {}",
                g.vision_range,
            );
        }
    }

    #[test]
    fn mutation_keeps_traits_in_bounds() {
        let mut rng = StdRng::seed_from_u64(7);
        // Génome aux bornes : on s'assure que la mutation ne déborde pas.
        let parent = Genome {
            speed: SPEED_MAX,
            size: SIZE_MIN,
            vision_range: VISION_MAX,
        };
        for _ in 0..1000 {
            let child = mutate_genome(&mut rng, &parent);
            assert!(child.speed >= SPEED_MIN && child.speed <= SPEED_MAX);
            assert!(child.size >= SIZE_MIN && child.size <= SIZE_MAX);
            assert!(child.vision_range >= VISION_MIN && child.vision_range <= VISION_MAX);
        }
    }

    #[test]
    fn mutation_usually_close_to_parent() {
        // Sur un grand nombre d'enfants, la moyenne doit rester proche du parent.
        let mut rng = StdRng::seed_from_u64(123);
        let parent = Genome {
            speed: 100.0,
            size: 6.0,
            vision_range: 80.0,
        };
        let n = 5000;
        let mut sum_speed = 0.0_f64;
        for _ in 0..n {
            sum_speed += mutate_genome(&mut rng, &parent).speed as f64;
        }
        let mean = sum_speed / n as f64;
        // Avec MUTATION_RATE ~ 0.18 et delta de moyenne nulle, la moyenne doit
        // rester très proche du parent (à quelques %).
        assert!((mean - 100.0).abs() < 3.0, "mean speed drifted: {mean}");
    }
}
