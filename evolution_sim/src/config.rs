//! Constantes globales de la simulation d'évolution darwinienne.
//!
//! Ce fichier centralise tous les paramètres « magiques » de la simulation
//! pour permettre un tuning rapide sans toucher au reste du code.

// ───────────────────────────── Fenêtre / monde ───────────────────────────────

pub const WINDOW_WIDTH: f32 = 1400.0;
pub const WINDOW_HEIGHT: f32 = 900.0;
pub const HALF_WIDTH: f32 = WINDOW_WIDTH * 0.5;
pub const HALF_HEIGHT: f32 = WINDOW_HEIGHT * 0.5;

/// Marge intérieure : zone effective où évoluent les organismes.
pub const WORLD_MARGIN: f32 = 30.0;
pub const WORLD_HALF_WIDTH: f32 = HALF_WIDTH - WORLD_MARGIN;
pub const WORLD_HALF_HEIGHT: f32 = HALF_HEIGHT - WORLD_MARGIN;

// ───────────────────────────── Cadence physique ──────────────────────────────

pub const SIM_HZ: f64 = 60.0;

// ───────────────────────────── Population initiale ───────────────────────────

pub const INITIAL_POPULATION: usize = 60;
pub const INITIAL_FOOD: usize = 220;

/// Bornes douces sur la population pour éviter explosion / extinction silencieuse.
pub const SOFT_POPULATION_CAP: usize = 600;

// ───────────────────────────── Génétique ─────────────────────────────────────

/// Trait : vitesse maximale (pixels / seconde). Moyenne et écart-type pour la
/// génération initiale (distribution normale tronquée à [min, max]).
pub const SPEED_INIT_MEAN: f32 = 70.0;
pub const SPEED_INIT_STD: f32 = 20.0;
pub const SPEED_MIN: f32 = 10.0;
pub const SPEED_MAX: f32 = 220.0;

/// Trait : rayon de l'organisme (pixels). Influence coût énergétique et
/// rayon de capture de la nourriture.
pub const SIZE_INIT_MEAN: f32 = 6.0;
pub const SIZE_INIT_STD: f32 = 1.5;
pub const SIZE_MIN: f32 = 2.5;
pub const SIZE_MAX: f32 = 18.0;

/// Trait : portée de vision (pixels). Permet de détecter la nourriture proche.
pub const VISION_INIT_MEAN: f32 = 80.0;
pub const VISION_INIT_STD: f32 = 25.0;
pub const VISION_MIN: f32 = 20.0;
pub const VISION_MAX: f32 = 260.0;

/// Probabilité (par trait) qu'une mutation survienne à la reproduction.
pub const MUTATION_RATE: f32 = 0.18;
/// Amplitude relative d'une mutation (écart-type, en fraction du trait parent).
pub const MUTATION_SCALE: f32 = 0.18;

// ───────────────────────────── Énergie ───────────────────────────────────────

pub const ENERGY_INIT: f32 = 100.0;
pub const ENERGY_MAX: f32 = 220.0;

/// Énergie apportée par une particule de nourriture.
pub const FOOD_ENERGY: f32 = 38.0;

/// Coût métabolique de base par seconde (indépendant du mouvement). Pénalise
/// les organismes inutilement gros.
pub const BASAL_COST_PER_SEC: f32 = 1.4;
pub const SIZE_METABOLIC_COEF: f32 = 0.10;

/// Coût de déplacement : k * vitesse² par seconde (énergie ∝ travail).
pub const MOVEMENT_COST_COEF: f32 = 1.6e-3;

/// Coût de la vision : un meilleur "capteur" coûte un peu plus cher.
pub const VISION_COST_COEF: f32 = 6e-3;

/// Énergie minimale pour se reproduire.
pub const REPRODUCTION_THRESHOLD: f32 = 170.0;
/// Énergie consommée à la reproduction (transférée à l'enfant).
pub const REPRODUCTION_COST: f32 = 90.0;
/// Énergie initiale de l'enfant (fraction du coût de reproduction).
pub const OFFSPRING_ENERGY_RATIO: f32 = 0.70;
/// Délai minimum entre deux reproductions (secondes).
pub const REPRODUCTION_COOLDOWN: f32 = 2.0;

// ───────────────────────────── Comportement ──────────────────────────────────

/// Au-delà de cette distance (relative à la vision), l'organisme erre.
pub const WANDER_TURN_RATE: f32 = 1.2;
/// Lissage de la direction (taux par seconde vers la cible).
pub const STEERING_RATE: f32 = 6.0;

// ───────────────────────────── Nourriture ────────────────────────────────────

pub const FOOD_HALF_SIZE: f32 = 2.5;
/// Cadence de respawn de la nourriture (particules par seconde).
pub const FOOD_RESPAWN_RATE: f32 = 28.0;
/// Cible de nourriture présente à un instant donné.
pub const FOOD_TARGET: usize = 280;

// ───────────────────────────── Rendu ─────────────────────────────────────────

pub const Z_FOOD: f32 = 0.0;
pub const Z_ORGANISM: f32 = 1.0;
