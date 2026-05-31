//! Jeux de données jouets, choisis pour rendre l'apprentissage *visible*.
//!
//! Les trois problèmes de classification vivent dans l'espace d'entrée 2D
//! [-1, 1]² : on peut donc afficher en fond la frontière de décision qui se
//! déforme au fil de l'entraînement. La régression 1D montre, elle, la courbe
//! prédite se rapprocher de la cible.

use rand::Rng;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DatasetKind {
    Xor,
    Spirals,
    Circles,
    Regression,
}

impl DatasetKind {
    pub fn label(self) -> &'static str {
        match self {
            DatasetKind::Xor => "XOR",
            DatasetKind::Spirals => "Spirales",
            DatasetKind::Circles => "Cercles",
            DatasetKind::Regression => "Regression sin",
        }
    }
}

/// Un exemple d'entraînement : entrée `x`, cible `y`, et `class` pour la couleur.
pub struct Sample {
    pub x: Vec<f32>,
    pub y: Vec<f32>,
    pub class: u8,
}

pub struct Dataset {
    pub kind: DatasetKind,
    pub samples: Vec<Sample>,
    pub in_dim: usize,
    pub out_dim: usize,
}

impl Dataset {
    pub fn is_regression(&self) -> bool {
        self.kind == DatasetKind::Regression
    }

    pub fn new(kind: DatasetKind) -> Self {
        match kind {
            DatasetKind::Xor => Self::xor(),
            DatasetKind::Spirals => Self::spirals(),
            DatasetKind::Circles => Self::circles(),
            DatasetKind::Regression => Self::regression(),
        }
    }

    /// Le classique : 4 coins, classe = XOR des coordonnées.
    fn xor() -> Self {
        let pts = [
            (-0.8, -0.8, 0u8),
            (-0.8, 0.8, 1u8),
            (0.8, -0.8, 1u8),
            (0.8, 0.8, 0u8),
        ];
        let samples = pts
            .iter()
            .map(|&(x, y, c)| Sample {
                x: vec![x, y],
                y: vec![c as f32],
                class: c,
            })
            .collect();
        Dataset {
            kind: DatasetKind::Xor,
            samples,
            in_dim: 2,
            out_dim: 1,
        }
    }

    /// Deux spirales entrelacées : non linéairement séparables, très visuel.
    fn spirals() -> Self {
        let mut rng = rand::thread_rng();
        let per_class = 100;
        let mut samples = Vec::with_capacity(per_class * 2);
        for class in 0..2u8 {
            for k in 0..per_class {
                let r = k as f32 / per_class as f32;
                let t = 1.5 * r * std::f32::consts::TAU
                    + class as f32 * std::f32::consts::PI
                    + rng.gen_range(-0.15..0.15);
                let x = r * t.cos() * 0.9;
                let y = r * t.sin() * 0.9;
                samples.push(Sample {
                    x: vec![x, y],
                    y: vec![class as f32],
                    class,
                });
            }
        }
        Dataset {
            kind: DatasetKind::Spirals,
            samples,
            in_dim: 2,
            out_dim: 1,
        }
    }

    /// Disque intérieur (classe 0) vs anneau extérieur (classe 1).
    fn circles() -> Self {
        let mut rng = rand::thread_rng();
        let per_class = 100;
        let mut samples = Vec::with_capacity(per_class * 2);
        for class in 0..2u8 {
            for _ in 0..per_class {
                let radius = if class == 0 {
                    rng.gen_range(0.0..0.38)
                } else {
                    rng.gen_range(0.6..0.92)
                };
                let t = rng.gen_range(0.0..std::f32::consts::TAU);
                samples.push(Sample {
                    x: vec![radius * t.cos(), radius * t.sin()],
                    y: vec![class as f32],
                    class,
                });
            }
        }
        Dataset {
            kind: DatasetKind::Circles,
            samples,
            in_dim: 2,
            out_dim: 1,
        }
    }

    /// Régression 1D : approximer y = 0.7·sin(π·x) sur [-1, 1].
    fn regression() -> Self {
        let n = 48;
        let samples = (0..n)
            .map(|k| {
                let x = -1.0 + 2.0 * k as f32 / (n - 1) as f32;
                let y = 0.7 * (std::f32::consts::PI * x).sin();
                Sample {
                    x: vec![x],
                    y: vec![y],
                    class: 0,
                }
            })
            .collect();
        Dataset {
            kind: DatasetKind::Regression,
            samples,
            in_dim: 1,
            out_dim: 1,
        }
    }
}
