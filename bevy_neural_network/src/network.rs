//! Perceptron multicouche (MLP) « fait main », sans bibliothèque externe.
//!
//! On garde un contrôle total sur la mécanique pour pouvoir l'animer phase par
//! phase : la propagation avant (forward) remplit les activations `a` et les
//! pré-activations `z`, la rétropropagation (backward) remplit les erreurs
//! `d` (les deltas), et `apply_grad` applique la descente de gradient.
//!
//! Convention d'indices : `w[l][j][i]` est le poids reliant le neurone `i` de
//! la couche `l` au neurone `j` de la couche `l + 1`. Les couches cachées
//! utilisent `tanh`, la couche de sortie `sigmoid` (classification) ou
//! l'identité (régression).

use rand::Rng;

/// Fonction d'activation de la couche de sortie (dépend du problème).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum OutAct {
    Sigmoid,
    Linear,
}

/// Un réseau de neurones entièrement connecté.
pub struct Network {
    /// Nombre de neurones par couche, entrée et sortie comprises.
    pub sizes: Vec<usize>,
    /// Poids `w[l][j][i]`, pour `l` dans `0..L-1`.
    pub w: Vec<Vec<Vec<f32>>>,
    /// Biais `b[l][j]`, pour `l` dans `0..L-1`.
    pub b: Vec<Vec<f32>>,
    /// Activations `a[l]` (sortie de chaque neurone après activation).
    pub a: Vec<Vec<f32>>,
    /// Pré-activations `z[l]` (somme pondérée avant activation). `z[0]` inutilisé.
    pub z: Vec<Vec<f32>>,
    /// Deltas `d[l]` (erreur rétropropagée sur chaque neurone).
    pub d: Vec<Vec<f32>>,
    pub out: OutAct,
}

fn tanh(x: f32) -> f32 {
    x.tanh()
}

fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

impl Network {
    /// Construit un réseau avec une initialisation de Xavier (poids petits,
    /// échelle ~ 1/sqrt(fan_in)) pour démarrer l'apprentissage sereinement.
    pub fn new(sizes: &[usize], out: OutAct) -> Self {
        let mut rng = rand::thread_rng();
        let l = sizes.len();
        let mut w = Vec::with_capacity(l - 1);
        let mut b = Vec::with_capacity(l - 1);
        for k in 0..l - 1 {
            let fan_in = sizes[k];
            let scale = 1.0 / (fan_in as f32).sqrt();
            let layer_w: Vec<Vec<f32>> = (0..sizes[k + 1])
                .map(|_| {
                    (0..fan_in)
                        .map(|_| rng.gen_range(-scale..scale))
                        .collect()
                })
                .collect();
            let layer_b: Vec<f32> = (0..sizes[k + 1]).map(|_| 0.0).collect();
            w.push(layer_w);
            b.push(layer_b);
        }
        let zeros = |sizes: &[usize]| -> Vec<Vec<f32>> {
            sizes.iter().map(|&n| vec![0.0; n]).collect()
        };
        Network {
            sizes: sizes.to_vec(),
            w,
            b,
            a: zeros(sizes),
            z: zeros(sizes),
            d: zeros(sizes),
            out,
        }
    }

    fn out_apply(&self, x: f32) -> f32 {
        match self.out {
            OutAct::Sigmoid => sigmoid(x),
            OutAct::Linear => x,
        }
    }

    /// Propagation avant qui *stocke* `a` et `z` (pour l'échantillon animé).
    pub fn forward(&mut self, input: &[f32]) {
        let l = self.sizes.len();
        for i in 0..self.sizes[0] {
            self.a[0][i] = input.get(i).copied().unwrap_or(0.0);
        }
        for layer in 1..l {
            for j in 0..self.sizes[layer] {
                let mut z = self.b[layer - 1][j];
                for i in 0..self.sizes[layer - 1] {
                    z += self.w[layer - 1][j][i] * self.a[layer - 1][i];
                }
                self.z[layer][j] = z;
                self.a[layer][j] = if layer == l - 1 {
                    self.out_apply(z)
                } else {
                    tanh(z)
                };
            }
        }
    }

    /// Propagation avant « pure » qui ne touche pas l'état stocké : sert à
    /// recalculer la frontière de décision et la loss sur tout le jeu de données.
    pub fn forward_pure(&self, input: &[f32]) -> Vec<f32> {
        let l = self.sizes.len();
        let mut a: Vec<f32> = (0..self.sizes[0])
            .map(|i| input.get(i).copied().unwrap_or(0.0))
            .collect();
        for layer in 1..l {
            let mut na = vec![0.0; self.sizes[layer]];
            for j in 0..self.sizes[layer] {
                let mut z = self.b[layer - 1][j];
                for i in 0..self.sizes[layer - 1] {
                    z += self.w[layer - 1][j][i] * a[i];
                }
                na[j] = if layer == l - 1 {
                    self.out_apply(z)
                } else {
                    tanh(z)
                };
            }
            a = na;
        }
        a
    }

    /// Rétropropagation : calcule les deltas `d` à partir d'un forward déjà fait.
    /// Avec une loss MSE = 0.5·Σ(a − y)², on a dL/dz_sortie = (a − y)·act'(z).
    pub fn backward(&mut self, target: &[f32]) {
        let l = self.sizes.len();
        let last = l - 1;
        for j in 0..self.sizes[last] {
            let aj = self.a[last][j];
            let y = target.get(j).copied().unwrap_or(0.0);
            self.d[last][j] = match self.out {
                // sigmoid' = a·(1 − a)
                OutAct::Sigmoid => (aj - y) * aj * (1.0 - aj),
                // identité' = 1
                OutAct::Linear => aj - y,
            };
        }
        // Couches cachées : tanh' = 1 − a²
        for layer in (1..last).rev() {
            for i in 0..self.sizes[layer] {
                let mut s = 0.0;
                for j in 0..self.sizes[layer + 1] {
                    s += self.w[layer][j][i] * self.d[layer + 1][j];
                }
                let ai = self.a[layer][i];
                self.d[layer][i] = s * (1.0 - ai * ai);
            }
        }
    }

    /// Met à jour poids et biais par descente de gradient (SGD), à partir des
    /// deltas et activations courants : w ← w − lr·δ·a, b ← b − lr·δ.
    pub fn apply_grad(&mut self, lr: f32) {
        let l = self.sizes.len();
        for layer in 0..l - 1 {
            for j in 0..self.sizes[layer + 1] {
                let dj = self.d[layer + 1][j];
                self.b[layer][j] -= lr * dj;
                for i in 0..self.sizes[layer] {
                    self.w[layer][j][i] -= lr * dj * self.a[layer][i];
                }
            }
        }
    }
}
