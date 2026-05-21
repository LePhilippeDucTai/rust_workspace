//! Physique : gravité, intégration, collisions particule-piquet,
//! particule-mur, particule-cloison, particule-particule (grille spatiale).

use bevy::prelude::*;

use crate::components::{Divider, Particle, Peg, Velocity, Wall};
use crate::config::{
    DIVIDER_HALF_WIDTH, DIVIDER_RESTITUTION, GRAVITY, HALF_HEIGHT, HALF_WIDTH,
    LINEAR_DAMPING_PER_SEC, PARTICLE_RESTITUTION, PEG_JITTER, PEG_RESTITUTION, PHYSICS_SUBSTEPS,
};
use crate::resources::{BoardDims, not_paused};

pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedUpdate, step_physics.run_if(not_paused));
    }
}

/// Un seul système : on fait `PHYSICS_SUBSTEPS` sous-pas pour stabiliser les
/// piles denses dans les bacs (sinon les particules s'enfoncent dans le sol).
fn step_physics(
    time: Res<Time<Fixed>>,
    dims: Res<BoardDims>,
    mut particles: Query<(&mut Transform, &mut Velocity, &Particle)>,
    pegs: Query<(&Transform, &Peg), Without<Particle>>,
    walls: Query<&Wall>,
    dividers: Query<&Divider>,
) {
    let dt_total = time.delta_secs();
    let n = PHYSICS_SUBSTEPS as f32;
    let dt = dt_total / n;
    let damping = (-LINEAR_DAMPING_PER_SEC * dt).exp();

    // Pré-extraction des piquets en buffers compacts (lecture seule).
    let peg_data: Vec<(Vec2, f32)> = pegs
        .iter()
        .map(|(t, p)| (t.translation.truncate(), p.radius))
        .collect();
    let walls_vec: Vec<Wall> = walls.iter().cloned().collect();
    let dividers_vec: Vec<Divider> = dividers.iter().cloned().collect();

    // SoA pour les particules : on copie une fois, on travaille sur ces
    // buffers pendant tous les sous-pas, on réécrit à la fin via l'ordre
    // stable de la même Query.
    let count = particles.iter().count();
    let mut positions: Vec<Vec2> = Vec::with_capacity(count);
    let mut velocities: Vec<Vec2> = Vec::with_capacity(count);
    let mut radii: Vec<f32> = Vec::with_capacity(count);
    for (t, v, p) in particles.iter() {
        positions.push(t.translation.truncate());
        velocities.push(v.0);
        radii.push(p.radius);
    }

    for _substep in 0..PHYSICS_SUBSTEPS {
        // Gravité + intégration + damping.
        for i in 0..positions.len() {
            velocities[i].y -= GRAVITY * dt;
            velocities[i] *= damping;
            positions[i] += velocities[i] * dt;
        }
        // Collisions particule ↔ piquet.
        resolve_pegs(&mut positions, &mut velocities, &radii, &peg_data);
        // Collisions particule ↔ cloison.
        resolve_dividers(&mut positions, &mut velocities, &radii, &dividers_vec);
        // Collisions particule ↔ mur (et sol).
        resolve_walls(&mut positions, &mut velocities, &radii, &walls_vec);
        // Bornes globales (sécurité contre la fuite hors fenêtre).
        clamp_world(&mut positions, &mut velocities, &radii, &dims);
        // Collisions particule ↔ particule.
        resolve_particles(&mut positions, &mut velocities, &radii);
    }

    // Réécriture vers l'ECS dans l'ordre de la query (stable).
    for (i, (mut t, mut v, _)) in (&mut particles).into_iter().enumerate() {
        if i >= positions.len() {
            break;
        }
        t.translation.x = positions[i].x;
        t.translation.y = positions[i].y;
        v.0 = velocities[i];
    }
}

fn resolve_pegs(
    positions: &mut [Vec2],
    velocities: &mut [Vec2],
    radii: &[f32],
    pegs: &[(Vec2, f32)],
) {
    for i in 0..positions.len() {
        let r_p = radii[i];
        for &(peg_pos, peg_r) in pegs.iter() {
            let delta = positions[i] - peg_pos;
            let min_d = r_p + peg_r;
            if delta.x.abs() > min_d || delta.y.abs() > min_d {
                continue;
            }
            let dist_sq = delta.length_squared();
            if dist_sq >= min_d * min_d {
                continue;
            }
            let mut normal = if dist_sq < 1e-8 {
                // Particule pile sur le centre du piquet : pousse vers le haut.
                Vec2::new(0.0, 1.0)
            } else {
                delta / dist_sq.sqrt()
            };
            // Petit aléa pour briser les équilibres précaires au sommet du piquet.
            if normal.y > 0.985 {
                let seed = (positions[i].x * 91.0 + positions[i].y * 17.0).sin();
                normal.x += seed * PEG_JITTER;
                normal = normal.normalize();
            }
            positions[i] = peg_pos + normal * (min_d + 1e-4);
            let v = velocities[i];
            let vn = v.dot(normal);
            if vn < 0.0 {
                velocities[i] = v - normal * ((1.0 + PEG_RESTITUTION) * vn);
            }
        }
    }
}

fn resolve_dividers(
    positions: &mut [Vec2],
    velocities: &mut [Vec2],
    radii: &[f32],
    dividers: &[Divider],
) {
    for i in 0..positions.len() {
        let r = radii[i];
        for d in dividers {
            // Test contre le rectangle [x±DIVIDER_HALF_WIDTH] × [y_min, y_max].
            let p = positions[i];
            let dx = p.x - d.x;
            let cy = (d.y_min + d.y_max) * 0.5;
            let half_h = (d.y_max - d.y_min) * 0.5;
            let dy = p.y - cy;
            // Point du rectangle le plus proche de la particule.
            let closest_x = dx.clamp(-DIVIDER_HALF_WIDTH, DIVIDER_HALF_WIDTH);
            let closest_y = dy.clamp(-half_h, half_h);
            let to_x = dx - closest_x;
            let to_y = dy - closest_y;
            let dist_sq = to_x * to_x + to_y * to_y;
            if dist_sq >= r * r {
                continue;
            }
            let (normal, overlap) = if dist_sq > 1e-8 {
                let dist = dist_sq.sqrt();
                (Vec2::new(to_x / dist, to_y / dist), r - dist)
            } else {
                let px = DIVIDER_HALF_WIDTH - dx.abs();
                let py = half_h - dy.abs();
                if px < py {
                    let sx = if dx >= 0.0 { 1.0 } else { -1.0 };
                    (Vec2::new(sx, 0.0), px + r)
                } else {
                    let sy = if dy >= 0.0 { 1.0 } else { -1.0 };
                    (Vec2::new(0.0, sy), py + r)
                }
            };
            positions[i].x += normal.x * (overlap + 1e-4);
            positions[i].y += normal.y * (overlap + 1e-4);
            let v = velocities[i];
            let vn = v.dot(normal);
            if vn < 0.0 {
                velocities[i] = v - normal * ((1.0 + DIVIDER_RESTITUTION) * vn);
            }
        }
    }
}

fn resolve_walls(
    positions: &mut [Vec2],
    velocities: &mut [Vec2],
    radii: &[f32],
    walls: &[Wall],
) {
    for i in 0..positions.len() {
        let r = radii[i];
        for w in walls {
            let p = positions[i];
            let dx = p.x - w.center.x;
            let dy = p.y - w.center.y;
            let closest_x = dx.clamp(-w.half_extents.x, w.half_extents.x);
            let closest_y = dy.clamp(-w.half_extents.y, w.half_extents.y);
            let to_x = dx - closest_x;
            let to_y = dy - closest_y;
            let dist_sq = to_x * to_x + to_y * to_y;
            if dist_sq >= r * r {
                continue;
            }
            let (normal, overlap) = if dist_sq > 1e-8 {
                let dist = dist_sq.sqrt();
                (Vec2::new(to_x / dist, to_y / dist), r - dist)
            } else {
                let px = w.half_extents.x - dx.abs();
                let py = w.half_extents.y - dy.abs();
                if px < py {
                    let sx = if dx >= 0.0 { 1.0 } else { -1.0 };
                    (Vec2::new(sx, 0.0), w.half_extents.x + r - dx.abs())
                } else {
                    let sy = if dy >= 0.0 { 1.0 } else { -1.0 };
                    (Vec2::new(0.0, sy), w.half_extents.y + r - dy.abs())
                }
            };
            positions[i].x += normal.x * (overlap + 1e-4);
            positions[i].y += normal.y * (overlap + 1e-4);
            let v = velocities[i];
            let vn = v.dot(normal);
            if vn < 0.0 {
                velocities[i] = v - normal * ((1.0 + w.restitution) * vn);
            }
        }
    }
}

/// Empêche les particules de sortir de la fenêtre (sécurité au cas où le sol
/// ou les murs auraient laissé filer).
fn clamp_world(
    positions: &mut [Vec2],
    velocities: &mut [Vec2],
    radii: &[f32],
    _dims: &BoardDims,
) {
    let half_w = HALF_WIDTH;
    let half_h = HALF_HEIGHT;
    for i in 0..positions.len() {
        let r = radii[i];
        if positions[i].x < -half_w + r {
            positions[i].x = -half_w + r;
            if velocities[i].x < 0.0 {
                velocities[i].x = -velocities[i].x * 0.2;
            }
        } else if positions[i].x > half_w - r {
            positions[i].x = half_w - r;
            if velocities[i].x > 0.0 {
                velocities[i].x = -velocities[i].x * 0.2;
            }
        }
        if positions[i].y < -half_h + r {
            positions[i].y = -half_h + r;
            if velocities[i].y < 0.0 {
                velocities[i].y = -velocities[i].y * 0.2;
            }
        } else if positions[i].y > half_h - r {
            positions[i].y = half_h - r;
            if velocities[i].y > 0.0 {
                velocities[i].y = -velocities[i].y * 0.2;
            }
        }
    }
}

// ───────────────── Particule ↔ Particule via grille spatiale ─────────────────

fn resolve_particles(positions: &mut [Vec2], velocities: &mut [Vec2], radii: &[f32]) {
    let n = positions.len();
    if n < 2 {
        return;
    }
    // Taille de cellule = ~2 × rayon max (suffit pour ne tester que les voisins
    // immédiats).
    let mut max_r: f32 = 0.0;
    for &r in radii {
        if r > max_r {
            max_r = r;
        }
    }
    let cell = (max_r * 2.0 + 1.0).max(8.0);
    let inv_cell = 1.0 / cell;
    let cols = ((HALF_WIDTH * 2.0) * inv_cell).ceil() as i32 + 2;
    let rows = ((HALF_HEIGHT * 2.0) * inv_cell).ceil() as i32 + 2;
    let total_cells = (cols * rows) as usize;

    let to_cell = |p: Vec2| -> (i32, i32) {
        let cx = ((p.x + HALF_WIDTH) * inv_cell) as i32;
        let cy = ((p.y + HALF_HEIGHT) * inv_cell) as i32;
        (cx.clamp(0, cols - 1), cy.clamp(0, rows - 1))
    };

    let mut grid: Vec<Vec<u32>> = vec![Vec::new(); total_cells];
    for i in 0..n {
        let (cx, cy) = to_cell(positions[i]);
        let idx = (cx + cy * cols) as usize;
        grid[idx].push(i as u32);
    }

    // Pour chaque cellule, tester paires intra-cellule + voisins (dx>=0,
    // dy in -1..=1 sauf dx=0&dy<0) pour éviter de traiter chaque paire deux fois.
    for cy in 0..rows {
        for cx in 0..cols {
            let cell_idx = (cx + cy * cols) as usize;
            let cell = grid[cell_idx].clone();
            if cell.is_empty() {
                continue;
            }
            // Paires intra-cellule.
            for a in 0..cell.len() {
                for b in (a + 1)..cell.len() {
                    resolve_pair(cell[a] as usize, cell[b] as usize, positions, velocities, radii);
                }
            }
            // Voisins : (dx, dy) avec dx+dy*cols > 0 par ordre lexical (+1,-1),(+1,0),(+1,+1),(0,+1).
            for &(dx, dy) in &[(1, -1), (1, 0), (1, 1), (0, 1)] {
                let nx = cx + dx;
                let ny = cy + dy;
                if nx < 0 || ny < 0 || nx >= cols || ny >= rows {
                    continue;
                }
                let n_idx = (nx + ny * cols) as usize;
                let neighbor = &grid[n_idx];
                for &i in &cell {
                    for &j in neighbor {
                        resolve_pair(i as usize, j as usize, positions, velocities, radii);
                    }
                }
            }
        }
    }
}

#[inline]
fn resolve_pair(
    i: usize,
    j: usize,
    positions: &mut [Vec2],
    velocities: &mut [Vec2],
    radii: &[f32],
) {
    let pi = positions[i];
    let pj = positions[j];
    let delta = pj - pi;
    let r_sum = radii[i] + radii[j];
    let dist_sq = delta.length_squared();
    if dist_sq >= r_sum * r_sum || dist_sq < 1e-12 {
        return;
    }
    let dist = dist_sq.sqrt();
    let normal = delta / dist;
    let overlap = r_sum - dist;
    // Masses égales (radii similaires) : on partage la correction par moitié.
    let half = overlap * 0.5;
    positions[i] = pi - normal * half;
    positions[j] = pj + normal * half;
    let vi = velocities[i];
    let vj = velocities[j];
    let rel = vj - vi;
    let vn = rel.dot(normal);
    if vn < 0.0 {
        let impulse = normal * (1.0 + PARTICLE_RESTITUTION) * vn * 0.5;
        velocities[i] = vi + impulse;
        velocities[j] = vj - impulse;
    }
}
