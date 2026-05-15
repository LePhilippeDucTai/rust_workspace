use rand::rng;
use rand::seq::SliceRandom;

use crate::colors::N;
use crate::components::SudokuGame;

impl SudokuGame {
    pub fn new() -> Self {
        let solution = generate_solution();
        let mask = make_puzzle_mask(35);
        let (current, given) = (0..N).flat_map(|r| (0..N).map(move |c| (r, c))).fold(
            ([[0u8; N]; N], [[false; N]; N]),
            |(mut cur, mut giv), (r, c)| {
                if mask[r][c] {
                    cur[r][c] = solution[r][c];
                    giv[r][c] = true;
                }
                (cur, giv)
            },
        );
        Self { current, given, won: false, history: vec![], future: vec![] }
    }
}

pub fn has_conflict(b: &[[u8; N]; N], r: usize, c: usize) -> bool {
    let v = b[r][c];
    if v == 0 {
        return false;
    }
    let row_conflict = (0..N).filter(|&i| i != c).any(|i| b[r][i] == v);
    let col_conflict = (0..N).filter(|&i| i != r).any(|i| b[i][c] == v);
    let (br, bc) = ((r / 3) * 3, (c / 3) * 3);
    let box_conflict = (br..br + 3)
        .flat_map(|i| (bc..bc + 3).map(move |j| (i, j)))
        .filter(|&pos| pos != (r, c))
        .any(|(i, j)| b[i][j] == v);
    row_conflict || col_conflict || box_conflict
}

pub fn is_complete_and_valid(b: &[[u8; N]; N]) -> bool {
    (0..N)
        .flat_map(|r| (0..N).map(move |c| (r, c)))
        .all(|(r, c)| b[r][c] != 0 && !has_conflict(b, r, c))
}

fn generate_solution() -> [[u8; N]; N] {
    let base: [[u8; N]; N] = [
        [1, 2, 3, 4, 5, 6, 7, 8, 9],
        [4, 5, 6, 7, 8, 9, 1, 2, 3],
        [7, 8, 9, 1, 2, 3, 4, 5, 6],
        [2, 3, 1, 5, 6, 4, 8, 9, 7],
        [5, 6, 4, 8, 9, 7, 2, 3, 1],
        [8, 9, 7, 2, 3, 1, 5, 6, 4],
        [3, 1, 2, 6, 4, 5, 9, 7, 8],
        [6, 4, 5, 9, 7, 8, 3, 1, 2],
        [9, 7, 8, 3, 1, 2, 6, 4, 5],
    ];
    let mut rng = rng();

    let mut digits: [u8; 9] = [1, 2, 3, 4, 5, 6, 7, 8, 9];
    digits.shuffle(&mut rng);

    // Remap digits
    let g = base.map(|row| row.map(|v| digits[(v - 1) as usize]));

    // Shuffle rows within each horizontal band
    let g = shuffle_row_bands(g, &mut rng);
    // Shuffle columns within each vertical band
    let g = shuffle_col_bands(g, &mut rng);
    // Shuffle horizontal bands
    let g = shuffle_bands_h(g, &mut rng);
    // Shuffle vertical bands
    shuffle_bands_v(g, &mut rng)
}

fn shuffle_row_bands(mut g: [[u8; N]; N], rng: &mut impl rand::RngCore) -> [[u8; N]; N] {
    for band in 0..3 {
        let mut p = [0usize, 1, 2];
        p.shuffle(rng);
        let snapshot = g;
        for i in 0..3 {
            g[band * 3 + i] = snapshot[band * 3 + p[i]];
        }
    }
    g
}

fn shuffle_col_bands(mut g: [[u8; N]; N], rng: &mut impl rand::RngCore) -> [[u8; N]; N] {
    for band in 0..3 {
        let mut p = [0usize, 1, 2];
        p.shuffle(rng);
        let snapshot = g;
        for i in 0..N {
            for j in 0..3 {
                g[i][band * 3 + j] = snapshot[i][band * 3 + p[j]];
            }
        }
    }
    g
}

fn shuffle_bands_h(mut g: [[u8; N]; N], rng: &mut impl rand::RngCore) -> [[u8; N]; N] {
    let mut p = [0usize, 1, 2];
    p.shuffle(rng);
    let snapshot = g;
    for b in 0..3 {
        for i in 0..3 {
            g[b * 3 + i] = snapshot[p[b] * 3 + i];
        }
    }
    g
}

fn shuffle_bands_v(mut g: [[u8; N]; N], rng: &mut impl rand::RngCore) -> [[u8; N]; N] {
    let mut p = [0usize, 1, 2];
    p.shuffle(rng);
    let snapshot = g;
    for b in 0..3 {
        for j in 0..3 {
            for i in 0..N {
                g[i][b * 3 + j] = snapshot[i][p[b] * 3 + j];
            }
        }
    }
    g
}

fn make_puzzle_mask(clues: usize) -> [[bool; N]; N] {
    let mut positions: Vec<(usize, usize)> = (0..81).map(|i| (i / 9, i % 9)).collect();
    positions.shuffle(&mut rng());
    positions.iter().take(clues).fold([[false; N]; N], |mut mask, &(r, c)| {
        mask[r][c] = true;
        mask
    })
}
