//! Cœur du modèle de tas de sable abélien (Bak-Tang-Wiesenfeld, 1987).
//!
//! Chaque cellule d'une grille porte un nombre entier de grains. Dès qu'une
//! cellule atteint le seuil critique (4 = nombre de voisins), elle « s'effondre »
//! (topple) : elle perd 4 grains et en distribue 1 à chacun de ses 4 voisins.
//! Les grains qui sortent de la grille sont dissipés (bord ouvert).
//!
//! Deux propriétés rendent ce modèle fascinant :
//!   * **Abélien** : l'état final stable après ajout d'un grain ne dépend ni de
//!     l'ordre des effondrements, ni de la façon dont on les ordonnance. On peut
//!     donc effondrer en vrac sans changer le résultat.
//!   * **Criticité auto-organisée (SOC)** : en ajoutant des grains au hasard, le
//!     système se cale tout seul sur un état critique où la taille des avalanches
//!     suit une loi de puissance P(s) ∝ s^(-τ). Aucun paramètre n'est ajusté :
//!     la criticité émerge — d'où l'intérêt probabiliste.
//!
//! Ce module ne dépend pas de Bevy : il est testable isolément.

/// Seuil critique : nombre de grains au-delà duquel une cellule s'effondre.
pub const THRESHOLD: u32 = 4;

/// Résultat de la stabilisation déclenchée par l'ajout d'un grain.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Avalanche {
    /// Taille `s` : nombre total d'effondrements individuels (statistique SOC).
    pub topplings: u64,
    /// Aire : nombre de cellules distinctes ayant basculé au moins une fois.
    pub area: u32,
}

/// Grille de tas de sable avec tampons de travail réutilisés entre avalanches.
pub struct Sandpile {
    pub w: usize,
    pub h: usize,
    /// Nombre de grains par cellule (indexée `y * w + x`).
    pub cells: Vec<u32>,
    /// Pile des cellules candidates instables (réutilisée, évite les allocations).
    stack: Vec<usize>,
    /// Marqueur « a déjà basculé pendant cette avalanche » (pour l'aire).
    toppled: Vec<bool>,
    /// Liste des cellules marquées, pour réinitialiser `toppled` à coût minimal.
    toppled_list: Vec<usize>,
}

impl Sandpile {
    pub fn new(w: usize, h: usize) -> Self {
        Self {
            w,
            h,
            cells: vec![0; w * h],
            stack: Vec::new(),
            toppled: vec![false; w * h],
            toppled_list: Vec::new(),
        }
    }

    #[inline]
    pub fn idx(&self, x: usize, y: usize) -> usize {
        y * self.w + x
    }

    /// Index de la cellule centrale (utile pour le mode « pile centrale »).
    #[inline]
    pub fn center(&self) -> usize {
        self.idx(self.w / 2, self.h / 2)
    }

    /// Masse totale (somme des grains présents) — tend vers ~2,1/cellule au seuil.
    pub fn mass(&self) -> u64 {
        self.cells.iter().map(|&c| c as u64).sum()
    }

    pub fn clear(&mut self) {
        self.cells.iter_mut().for_each(|c| *c = 0);
    }

    /// Ajoute un grain en `i` puis stabilise toute la grille, en renvoyant les
    /// caractéristiques de l'avalanche déclenchée.
    ///
    /// On effondre « en vrac » : une cellule à `c` grains bascule `c / 4` fois
    /// d'un coup. C'est correct grâce à la propriété abélienne et bien plus
    /// rapide qu'un effondrement unitaire.
    pub fn add_grain(&mut self, i: usize) -> Avalanche {
        self.cells[i] += 1;
        self.toppled_list.clear();
        let mut topplings: u64 = 0;

        self.stack.clear();
        if self.cells[i] >= THRESHOLD {
            self.stack.push(i);
        }

        while let Some(c) = self.stack.pop() {
            let count = self.cells[c];
            if count < THRESHOLD {
                continue;
            }
            let t = count / THRESHOLD; // nombre d'effondrements en vrac
            self.cells[c] = count - t * THRESHOLD;
            topplings += t as u64;

            if !self.toppled[c] {
                self.toppled[c] = true;
                self.toppled_list.push(c);
            }

            let x = c % self.w;
            let y = c / self.w;
            // Distribue `t` grains à chaque voisin présent dans la grille ;
            // ceux dirigés hors de la grille sont dissipés (bord ouvert).
            if x > 0 {
                let n = c - 1;
                self.cells[n] += t;
                if self.cells[n] >= THRESHOLD {
                    self.stack.push(n);
                }
            }
            if x + 1 < self.w {
                let n = c + 1;
                self.cells[n] += t;
                if self.cells[n] >= THRESHOLD {
                    self.stack.push(n);
                }
            }
            if y > 0 {
                let n = c - self.w;
                self.cells[n] += t;
                if self.cells[n] >= THRESHOLD {
                    self.stack.push(n);
                }
            }
            if y + 1 < self.h {
                let n = c + self.w;
                self.cells[n] += t;
                if self.cells[n] >= THRESHOLD {
                    self.stack.push(n);
                }
            }
        }

        let area = self.toppled_list.len() as u32;
        for &c in &self.toppled_list {
            self.toppled[c] = false;
        }

        Avalanche { topplings, area }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn under_threshold_no_avalanche() {
        let mut p = Sandpile::new(5, 5);
        let c = p.center();
        for _ in 0..3 {
            let av = p.add_grain(c);
            assert_eq!(av.topplings, 0);
        }
        assert_eq!(p.cells[c], 3);
    }

    #[test]
    fn single_topple_distributes_to_four_neighbors() {
        let mut p = Sandpile::new(5, 5);
        let c = p.center();
        p.cells[c] = 3;
        let av = p.add_grain(c); // atteint 4 -> bascule une fois
        assert_eq!(av.topplings, 1);
        assert_eq!(av.area, 1);
        assert_eq!(p.cells[c], 0);
        // Chacun des 4 voisins a reçu exactement 1 grain.
        assert_eq!(p.cells[p.idx(1, 2)], 1);
        assert_eq!(p.cells[p.idx(3, 2)], 1);
        assert_eq!(p.cells[p.idx(2, 1)], 1);
        assert_eq!(p.cells[p.idx(2, 3)], 1);
    }

    #[test]
    fn boundary_grains_are_dissipated() {
        // Un coin n'a que 2 voisins : 2 grains sortent de la grille.
        let mut p = Sandpile::new(3, 3);
        p.cells[0] = 3;
        let av = p.add_grain(0);
        assert_eq!(av.topplings, 1);
        assert_eq!(p.cells[0], 0);
        assert_eq!(p.cells[p.idx(1, 0)], 1);
        assert_eq!(p.cells[p.idx(0, 1)], 1);
        // Masse passée de 4 à 2 : 2 grains dissipés au bord.
        assert_eq!(p.mass(), 2);
    }

    #[test]
    fn always_stabilizes_below_threshold() {
        let mut p = Sandpile::new(31, 31);
        let c = p.center();
        for _ in 0..2000 {
            p.add_grain(c);
        }
        assert!(p.cells.iter().all(|&c| c < THRESHOLD));
    }

    #[test]
    fn abelian_order_independence() {
        // L'état stable final ne dépend pas de l'ordre d'ajout des grains.
        let n = 21;
        let seq_a = [10usize, 10, 10, 10, 220, 5, 100, 100, 100, 100];
        let mut a = Sandpile::new(n, n);
        for &i in &seq_a {
            a.add_grain(i);
        }
        let mut b = Sandpile::new(n, n);
        for &i in seq_a.iter().rev() {
            b.add_grain(i);
        }
        assert_eq!(a.cells, b.cells);
    }
}
