//! Chaîne de Markov : matrice de transition, échantillonnage, distribution
//! stationnaire.
//!
//! Le théorème de convergence ergodique garantit que pour une chaîne de
//! Markov **irréductible** (tous les états communiquent) et **apériodique**
//! (pas de cycles déterministes), la distribution P^n converge vers
//! l'unique distribution stationnaire π satisfaisant π·P = π.

use ndarray::{Array1, Array2};
use rand::Rng;

/// Une chaîne de Markov en temps discret.
#[derive(Clone, Debug)]
pub struct MarkovChain {
    /// Matrice de transition P (n×n), où P[i,j] = P(état_j | état_i).
    pub transition: Array2<f64>,
    /// Distribution stationnaire π satisfaisant π·P = π.
    pub stationary: Array1<f64>,
}

impl MarkovChain {
    pub fn new(transition: Array2<f64>) -> Self {
        let stationary = compute_stationary(&transition);
        Self {
            transition,
            stationary,
        }
    }

    pub fn num_states(&self) -> usize {
        self.transition.nrows()
    }

    /// Échantillonne le prochain état à partir de `current` selon les
    /// probabilités de transition. Méthode d'inversion CDF.
    pub fn sample_next<R: Rng>(&self, current: usize, rng: &mut R) -> usize {
        let row = self.transition.row(current);
        let r: f64 = rng.r#gen();
        let mut acc = 0.0;
        for (j, &p) in row.iter().enumerate() {
            acc += p;
            if r < acc {
                return j;
            }
        }
        self.num_states() - 1
    }
}

/// Calcule la distribution stationnaire par méthode de la puissance.
///
/// On itère π_{k+1} = π_k · P jusqu'à convergence. Pour une chaîne
/// irréductible apériodique, cela converge vers l'unique π.
fn compute_stationary(p: &Array2<f64>) -> Array1<f64> {
    let n = p.nrows();
    let mut pi = Array1::from_elem(n, 1.0 / n as f64);
    let max_iter = 10_000;
    let tol = 1e-10;

    for _ in 0..max_iter {
        let next = pi.dot(p);
        let diff: f64 = (&next - &pi).iter().map(|x| x.abs()).sum();
        pi = next;
        if diff < tol {
            break;
        }
    }

    let sum: f64 = pi.sum();
    if sum > 0.0 {
        pi.mapv_inplace(|x| x / sum);
    }
    pi
}

/// Chaîne de référence : 6 états, irréductible et apériodique.
///
/// Construite à partir de l'énoncé du projet :
/// - 0 → 1 (0.40), 0 → 2 (0.60)
/// - 1 → 0 (0.50), 1 → 3 (0.50)
/// - 2 → 4 (0.30), 2 → 5 (0.70)
/// - 3 → 0 (1.00)
/// - 4 → 1 (0.50), 4 → 3 (0.50)
/// - 5 → 2 (0.60), 5 → 4 (0.40)
///
/// Une petite quantité d'auto-transition est ajoutée pour garantir
/// l'apériodicité.
pub fn default_chain() -> MarkovChain {
    let n = 6;
    let mut m = Array2::<f64>::zeros((n, n));

    m[[0, 1]] = 0.40;
    m[[0, 2]] = 0.60;

    m[[1, 0]] = 0.50;
    m[[1, 3]] = 0.50;

    m[[2, 4]] = 0.30;
    m[[2, 5]] = 0.70;

    m[[3, 0]] = 1.00;

    m[[4, 1]] = 0.50;
    m[[4, 3]] = 0.50;

    m[[5, 2]] = 0.60;
    m[[5, 4]] = 0.40;

    // Lissage : 5% d'auto-transition pour garantir l'apériodicité,
    // 95% de la masse vers les transitions originales.
    let eps = 0.05;
    for i in 0..n {
        let row_sum: f64 = m.row(i).sum();
        if row_sum > 0.0 {
            for j in 0..n {
                m[[i, j]] *= (1.0 - eps) / row_sum;
            }
            m[[i, i]] += eps;
        }
    }

    MarkovChain::new(m)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stationary_satisfies_fixed_point() {
        let chain = default_chain();
        let pi = &chain.stationary;
        let next = pi.dot(&chain.transition);
        for i in 0..chain.num_states() {
            assert!((next[i] - pi[i]).abs() < 1e-8, "pi*P != pi at {i}");
        }
    }

    #[test]
    fn stationary_sums_to_one() {
        let chain = default_chain();
        let s: f64 = chain.stationary.sum();
        assert!((s - 1.0).abs() < 1e-8, "sum(pi) = {s}");
    }

    #[test]
    fn rows_are_stochastic() {
        let chain = default_chain();
        for i in 0..chain.num_states() {
            let row_sum: f64 = chain.transition.row(i).sum();
            assert!((row_sum - 1.0).abs() < 1e-8, "row {i} sums to {row_sum}");
        }
    }
}
