use bevy::prelude::*;
use rand::Rng;

/// Flat row-major grid of boolean cell states.
///
/// Topology is toroidal: the left/right and top/bottom edges wrap around,
/// so every cell always has exactly eight neighbours.
#[derive(Resource)]
pub struct Grid {
    cells: Vec<bool>,
    pub width: usize,
    pub height: usize,
}

impl Grid {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            cells: vec![false; width * height],
            width,
            height,
        }
    }

    pub fn randomize(&mut self, density: f64) {
        let mut rng = rand::thread_rng();
        for cell in &mut self.cells {
            *cell = rng.gen_bool(density);
        }
    }

    pub fn clear(&mut self) {
        self.cells.fill(false);
    }

    #[inline]
    pub fn get(&self, x: usize, y: usize) -> bool {
        self.cells[y * self.width + x]
    }

    #[inline]
    pub fn set(&mut self, x: usize, y: usize, alive: bool) {
        self.cells[y * self.width + x] = alive;
    }

    pub fn alive_count(&self) -> usize {
        self.cells.iter().filter(|&&c| c).count()
    }

    /// Advances the simulation by one generation using Conway's rules:
    /// - A live cell with 2 or 3 live neighbours survives.
    /// - A dead cell with exactly 3 live neighbours becomes alive.
    /// - All other cells die or remain dead.
    pub fn step(&mut self) {
        let mut next = self.cells.clone();
        for y in 0..self.height {
            for x in 0..self.width {
                let alive = self.get(x, y);
                let n = self.count_neighbours(x, y);
                next[y * self.width + x] =
                    matches!((alive, n), (true, 2) | (true, 3) | (false, 3));
            }
        }
        self.cells = next;
    }

    fn count_neighbours(&self, x: usize, y: usize) -> u8 {
        let mut count = 0u8;
        for dy in [-1i32, 0, 1] {
            for dx in [-1i32, 0, 1] {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let nx = (x as i32 + dx).rem_euclid(self.width as i32) as usize;
                let ny = (y as i32 + dy).rem_euclid(self.height as i32) as usize;
                count += self.get(nx, ny) as u8;
            }
        }
        count
    }
}
