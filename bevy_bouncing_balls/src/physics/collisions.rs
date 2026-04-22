//! Collisions balle-balle :
//! - **SoA** (Struct of Arrays) pour la localité cache.
//! - **Grille plate** (Vec indexé) au lieu de HashMap, recyclée via `Local`.
//! - **Parallélisation Rayon par graph coloring 3×3** : les cellules d'une
//!   même couleur sont à distance ≥ 3, donc leurs voisinages 3×3 sont
//!   disjoints → aucun data race lors de la résolution en parallèle.

use bevy::prelude::*;
use rayon::prelude::*;
use std::sync::OnceLock;

use crate::components::{Ball, Velocity};
use crate::config::{
    GRID_CELLS, GRID_HEIGHT, GRID_WIDTH, HALF_HEIGHT, HALF_WIDTH, INV_CELL, PARALLEL_THRESHOLD,
};
use crate::resources::SimSettings;

// ───────────────────────── SoA ─────────────────────────

#[derive(Default)]
pub(super) struct Balls {
    entities: Vec<Entity>,
    positions: Vec<Vec2>,
    velocities: Vec<Vec2>,
    radii: Vec<f32>,
    inv_masses: Vec<f32>,
}

impl Balls {
    fn len(&self) -> usize {
        self.entities.len()
    }

    fn load(&mut self, query: &Query<(Entity, &mut Transform, &mut Velocity, &Ball)>) {
        self.entities.clear();
        self.positions.clear();
        self.velocities.clear();
        self.radii.clear();
        self.inv_masses.clear();
        for (e, t, v, b) in query.iter() {
            self.entities.push(e);
            self.positions.push(t.translation.truncate());
            self.velocities.push(v.0);
            self.radii.push(b.radius);
            self.inv_masses.push(1.0 / b.mass);
        }
    }

    fn write_back(&self, query: &mut Query<(Entity, &mut Transform, &mut Velocity, &Ball)>) {
        for (idx, &entity) in self.entities.iter().enumerate() {
            if let Ok((_, mut t, mut v, _)) = query.get_mut(entity) {
                t.translation.x = self.positions[idx].x;
                t.translation.y = self.positions[idx].y;
                v.0 = self.velocities[idx];
            }
        }
    }
}

/// Pointeurs partagés vers les buffers SoA. Utilisé par la phase parallèle.
///
/// SAFETY : la parallélisation par graph coloring 3×3 garantit que les
/// cellules traitées simultanément ont des voisinages disjoints, donc
/// aucun thread ne mute jamais les mêmes indices.
struct SharedBalls {
    positions: *mut Vec2,
    velocities: *mut Vec2,
    radii: *const f32,
    inv_masses: *const f32,
}
unsafe impl Send for SharedBalls {}
unsafe impl Sync for SharedBalls {}

impl SharedBalls {
    fn from(b: &mut Balls) -> Self {
        Self {
            positions: b.positions.as_mut_ptr(),
            velocities: b.velocities.as_mut_ptr(),
            radii: b.radii.as_ptr(),
            inv_masses: b.inv_masses.as_ptr(),
        }
    }
}

// ───────────────────── Grille plate ─────────────────────

#[inline]
fn cell_index(pos: Vec2) -> usize {
    let cx = (((pos.x + HALF_WIDTH) * INV_CELL) as usize).min(GRID_WIDTH - 1);
    let cy = (((pos.y + HALF_HEIGHT) * INV_CELL) as usize).min(GRID_HEIGHT - 1);
    cx + cy * GRID_WIDTH
}

fn rebuild_grid(grid: &mut Vec<Vec<u32>>, balls: &Balls) {
    if grid.len() != GRID_CELLS {
        *grid = (0..GRID_CELLS).map(|_| Vec::with_capacity(4)).collect();
    }
    for cell in grid.iter_mut() {
        cell.clear();
    }
    for (i, &pos) in balls.positions.iter().enumerate() {
        grid[cell_index(pos)].push(i as u32);
    }
}

/// 9 groupes de cellules par couleur (cx%3, cy%3), calculés une fois pour toutes
/// puisque la grille a une taille fixe.
fn color_groups() -> &'static [Vec<usize>; 9] {
    static GROUPS: OnceLock<[Vec<usize>; 9]> = OnceLock::new();
    GROUPS.get_or_init(|| {
        let mut groups: [Vec<usize>; 9] = core::array::from_fn(|_| Vec::new());
        for c in 0..GRID_CELLS {
            let cx = c % GRID_WIDTH;
            let cy = c / GRID_WIDTH;
            let color = (cx % 3) + (cy % 3) * 3;
            groups[color].push(c);
        }
        groups
    })
}

// ─────────────────── Système principal ──────────────────

pub fn resolve_ball_collisions(
    settings: Res<SimSettings>,
    mut query: Query<(Entity, &mut Transform, &mut Velocity, &Ball)>,
    mut balls: Local<Balls>,
    mut grid: Local<Vec<Vec<u32>>>,
) {
    if !settings.collisions_enabled {
        return;
    }
    balls.load(&query);
    if balls.len() < 2 {
        return;
    }
    rebuild_grid(&mut grid, &balls);

    let shared = SharedBalls::from(&mut balls);
    if balls.len() < PARALLEL_THRESHOLD {
        resolve_sequential(&grid, &shared);
    } else {
        resolve_parallel(&grid, &shared);
    }

    balls.write_back(&mut query);
}

fn resolve_sequential(grid: &[Vec<u32>], shared: &SharedBalls) {
    for cell_idx in 0..GRID_CELLS {
        // SAFETY : exécution mono-thread, aucun alias mutable concurrent.
        unsafe { process_cell(cell_idx, grid, shared) };
    }
}

fn resolve_parallel(grid: &[Vec<u32>], shared: &SharedBalls) {
    // Chaque groupe de couleur est traité en parallèle. Les groupes sont
    // traités séquentiellement entre eux (barrière implicite de par_iter).
    for group in color_groups() {
        group.par_iter().for_each(|&cell_idx| {
            // SAFETY : cells du même groupe ≥ 3 apart ⇒ voisinages 3×3 disjoints.
            unsafe { process_cell(cell_idx, grid, shared) };
        });
    }
}

/// Traite la cellule `cell_idx` : pairs intra-cellule + pairs vers les
/// cellules voisines d'indice supérieur (pour ne traiter chaque paire
/// qu'une seule fois, même en séquentiel).
#[inline]
unsafe fn process_cell(cell_idx: usize, grid: &[Vec<u32>], b: &SharedBalls) {
    let cell = &grid[cell_idx];
    if cell.is_empty() {
        return;
    }
    let cx = (cell_idx % GRID_WIDTH) as isize;
    let cy = (cell_idx / GRID_WIDTH) as isize;

    // Paires intra-cellule (a < bb)
    for a in 0..cell.len() {
        for bb in (a + 1)..cell.len() {
            unsafe { resolve_pair(cell[a] as usize, cell[bb] as usize, b) };
        }
    }

    // Paires vers les voisins d'indice strictement supérieur
    for dy in -1..=1 {
        let ny = cy + dy;
        if ny < 0 || ny >= GRID_HEIGHT as isize {
            continue;
        }
        for dx in -1..=1 {
            let nx = cx + dx;
            if nx < 0 || nx >= GRID_WIDTH as isize {
                continue;
            }
            let neighbor_idx = nx as usize + ny as usize * GRID_WIDTH;
            if neighbor_idx <= cell_idx {
                continue;
            }
            let neighbor = &grid[neighbor_idx];
            for &i in cell {
                for &j in neighbor {
                    unsafe { resolve_pair(i as usize, j as usize, b) };
                }
            }
        }
    }
}

// ────────────────── Résolution d'une paire ──────────────────

#[inline]
unsafe fn resolve_pair(i: usize, j: usize, b: &SharedBalls) {
    let pos_i = unsafe { *b.positions.add(i) };
    let pos_j = unsafe { *b.positions.add(j) };
    let delta = pos_j - pos_i;
    let r_i = unsafe { *b.radii.add(i) };
    let r_j = unsafe { *b.radii.add(j) };
    let min_dist = r_i + r_j;
    let dist_sq = delta.length_squared();
    if dist_sq >= min_dist * min_dist || dist_sq < 1e-12 {
        return;
    }

    let inv_dist = dist_sq.sqrt().recip();
    let dist = dist_sq * inv_dist;
    let normal = delta * inv_dist;
    let overlap = min_dist - dist;

    let inv_m_i = unsafe { *b.inv_masses.add(i) };
    let inv_m_j = unsafe { *b.inv_masses.add(j) };
    let inv_sum = (inv_m_i + inv_m_j).recip();

    // Correction positionnelle : m_k / (m_i + m_j) = inv_m_other / (inv_m_i + inv_m_j)
    let correction = normal * (overlap * inv_sum);
    unsafe {
        *b.positions.add(i) = pos_i - correction * inv_m_i;
        *b.positions.add(j) = pos_j + correction * inv_m_j;
    }

    let vel_i = unsafe { *b.velocities.add(i) };
    let vel_j = unsafe { *b.velocities.add(j) };
    let rel_vel = vel_j - vel_i;
    let vel_along_normal = rel_vel.dot(normal);
    if vel_along_normal < 0.0 {
        let impulse = normal * (-2.0 * vel_along_normal * inv_sum);
        unsafe {
            *b.velocities.add(i) = vel_i - impulse * inv_m_i;
            *b.velocities.add(j) = vel_j + impulse * inv_m_j;
        }
    }
}
