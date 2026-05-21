//! Physique : gravité, intégration, collisions (piquets, murs, cloisons,
//! particule-particule). Grille spatiale pour les collisions P↔P.

use bevy::prelude::*;

use crate::components::{Divider, Particle, Peg, Velocity, Wall};
use crate::config::{
    DIVIDER_HALF_WIDTH, DIVIDER_RESTITUTION, GRAVITY, HALF_HEIGHT, HALF_WIDTH,
    LINEAR_DAMPING_PER_SEC, PARTICLE_RESTITUTION, PEG_JITTER, PEG_RESTITUTION, PHYSICS_SUBSTEPS,
};
use crate::resources::{not_paused, BoardDims};

pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedUpdate, step_physics.run_if(not_paused));
    }
}

fn step_physics(
    time: Res<Time<Fixed>>,
    dims: Res<BoardDims>,
    mut particles: Query<(&mut Transform, &mut Velocity, &Particle)>,
    pegs: Query<(&Transform, &Peg), Without<Particle>>,
    walls: Query<&Wall>,
    dividers: Query<&Divider>,
) {
    let dt = time.delta_secs() / PHYSICS_SUBSTEPS as f32;
    let damping = (-LINEAR_DAMPING_PER_SEC * dt).exp();
    let peg_data: Vec<(Vec2, f32)> = pegs
        .iter()
        .map(|(t, p)| (t.translation.truncate(), p.radius))
        .collect();
    let walls_vec: Vec<Wall> = walls.iter().cloned().collect();
    let dividers_vec: Vec<Divider> = dividers.iter().cloned().collect();
    let (mut pos, mut vel, rad) = extract_soa(&particles);
    for _ in 0..PHYSICS_SUBSTEPS {
        run_substep(
            &mut pos,
            &mut vel,
            &rad,
            dt,
            damping,
            &peg_data,
            &dividers_vec,
            &walls_vec,
            &dims,
        );
    }
    writeback_soa(&mut particles, &pos, &vel);
}

fn extract_soa(
    particles: &Query<(&mut Transform, &mut Velocity, &Particle)>,
) -> (Vec<Vec2>, Vec<Vec2>, Vec<f32>) {
    let mut pos = Vec::new();
    let mut vel = Vec::new();
    let mut rad = Vec::new();
    for (t, v, p) in particles.iter() {
        pos.push(t.translation.truncate());
        vel.push(v.0);
        rad.push(p.radius);
    }
    (pos, vel, rad)
}

fn writeback_soa(
    particles: &mut Query<(&mut Transform, &mut Velocity, &Particle)>,
    pos: &[Vec2],
    vel: &[Vec2],
) {
    for (i, (mut t, mut v, _)) in particles.iter_mut().enumerate() {
        if i >= pos.len() {
            break;
        }
        t.translation.x = pos[i].x;
        t.translation.y = pos[i].y;
        v.0 = vel[i];
    }
}

fn run_substep(
    pos: &mut [Vec2],
    vel: &mut [Vec2],
    rad: &[f32],
    dt: f32,
    damping: f32,
    pegs: &[(Vec2, f32)],
    dividers: &[Divider],
    walls: &[Wall],
    dims: &BoardDims,
) {
    integrate(pos, vel, dt, damping);
    resolve_pegs(pos, vel, rad, pegs);
    resolve_dividers(pos, vel, rad, dividers);
    resolve_walls(pos, vel, rad, walls);
    clamp_world(pos, vel, rad, dims);
    resolve_particles(pos, vel, rad);
}

fn integrate(pos: &mut [Vec2], vel: &mut [Vec2], dt: f32, damping: f32) {
    for i in 0..pos.len() {
        vel[i].y -= GRAVITY * dt;
        vel[i] *= damping;
        pos[i] += vel[i] * dt;
    }
}

// ────────────────────────────── Piquets ──────────────────────────────────────

fn peg_normal(delta: Vec2, dist_sq: f32, pos: Vec2) -> Vec2 {
    if dist_sq < 1e-8 {
        return Vec2::Y;
    }
    let mut n = delta / dist_sq.sqrt();
    if n.y > 0.985 {
        let seed = (pos.x * 91.0 + pos.y * 17.0).sin();
        n.x += seed * PEG_JITTER;
        n = n.normalize();
    }
    n
}

fn resolve_peg_for_particle(
    i: usize,
    peg_pos: Vec2,
    peg_r: f32,
    pos: &mut [Vec2],
    vel: &mut [Vec2],
    r_p: f32,
) {
    let delta = pos[i] - peg_pos;
    let min_d = r_p + peg_r;
    if delta.x.abs() > min_d || delta.y.abs() > min_d {
        return;
    }
    let dist_sq = delta.length_squared();
    if dist_sq >= min_d * min_d {
        return;
    }
    let normal = peg_normal(delta, dist_sq, pos[i]);
    pos[i] = peg_pos + normal * (min_d + 1e-4);
    let vn = vel[i].dot(normal);
    if vn < 0.0 {
        vel[i] -= normal * ((1.0 + PEG_RESTITUTION) * vn);
    }
}

fn resolve_pegs(pos: &mut [Vec2], vel: &mut [Vec2], rad: &[f32], pegs: &[(Vec2, f32)]) {
    for i in 0..pos.len() {
        for &(peg_pos, peg_r) in pegs {
            resolve_peg_for_particle(i, peg_pos, peg_r, pos, vel, rad[i]);
        }
    }
}

// ────────────────────────── Rectangles (murs & cloisons) ─────────────────────

fn rect_normal(
    dx: f32,
    dy: f32,
    hw: f32,
    hh: f32,
    to_x: f32,
    to_y: f32,
    dist_sq: f32,
    r: f32,
) -> (Vec2, f32) {
    if dist_sq > 1e-8 {
        let dist = dist_sq.sqrt();
        return (Vec2::new(to_x / dist, to_y / dist), r - dist);
    }
    let (px, py) = (hw - dx.abs(), hh - dy.abs());
    if px < py {
        (Vec2::new(dx.signum(), 0.0), px + r)
    } else {
        (Vec2::new(0.0, dy.signum()), py + r)
    }
}

fn resolve_rect_particle(
    i: usize,
    cx: f32,
    cy: f32,
    hw: f32,
    hh: f32,
    r: f32,
    pos: &mut [Vec2],
    vel: &mut [Vec2],
    rest: f32,
) {
    let (dx, dy) = (pos[i].x - cx, pos[i].y - cy);
    let (to_x, to_y) = (dx - dx.clamp(-hw, hw), dy - dy.clamp(-hh, hh));
    let dist_sq = to_x * to_x + to_y * to_y;
    if dist_sq >= r * r {
        return;
    }
    let (normal, overlap) = rect_normal(dx, dy, hw, hh, to_x, to_y, dist_sq, r);
    pos[i] += normal * (overlap + 1e-4);
    let vn = vel[i].dot(normal);
    if vn < 0.0 {
        vel[i] -= normal * ((1.0 + rest) * vn);
    }
}

fn resolve_dividers(pos: &mut [Vec2], vel: &mut [Vec2], rad: &[f32], dividers: &[Divider]) {
    for i in 0..pos.len() {
        for d in dividers {
            let (cy, hh) = ((d.y_min + d.y_max) * 0.5, (d.y_max - d.y_min) * 0.5);
            resolve_rect_particle(
                i,
                d.x,
                cy,
                DIVIDER_HALF_WIDTH,
                hh,
                rad[i],
                pos,
                vel,
                DIVIDER_RESTITUTION,
            );
        }
    }
}

fn resolve_walls(pos: &mut [Vec2], vel: &mut [Vec2], rad: &[f32], walls: &[Wall]) {
    for i in 0..pos.len() {
        for w in walls {
            resolve_rect_particle(
                i,
                w.center.x,
                w.center.y,
                w.half_extents.x,
                w.half_extents.y,
                rad[i],
                pos,
                vel,
                w.restitution,
            );
        }
    }
}

// ────────────────────────────── Clampage ─────────────────────────────────────

fn clamp_axis(p: &mut f32, v: &mut f32, lo: f32, hi: f32) {
    if *p < lo {
        *p = lo;
        if *v < 0.0 {
            *v = -*v * 0.2;
        }
    } else if *p > hi {
        *p = hi;
        if *v > 0.0 {
            *v = -*v * 0.2;
        }
    }
}

fn clamp_world(pos: &mut [Vec2], vel: &mut [Vec2], rad: &[f32], _dims: &BoardDims) {
    for i in 0..pos.len() {
        clamp_axis(
            &mut pos[i].x,
            &mut vel[i].x,
            -HALF_WIDTH + rad[i],
            HALF_WIDTH - rad[i],
        );
        clamp_axis(
            &mut pos[i].y,
            &mut vel[i].y,
            -HALF_HEIGHT + rad[i],
            HALF_HEIGHT - rad[i],
        );
    }
}

// ──────────────────── Particule ↔ Particule (grille spatiale) ────────────────

fn to_cell(p: Vec2, cols: i32, rows: i32, inv_cell: f32) -> usize {
    let cx = ((p.x + HALF_WIDTH) * inv_cell) as i32;
    let cy = ((p.y + HALF_HEIGHT) * inv_cell) as i32;
    (cx.clamp(0, cols - 1) + cy.clamp(0, rows - 1) * cols) as usize
}

fn build_grid(pos: &[Vec2], cols: i32, rows: i32, inv_cell: f32) -> Vec<Vec<u32>> {
    let mut grid = vec![Vec::new(); (cols * rows) as usize];
    for (i, &p) in pos.iter().enumerate() {
        grid[to_cell(p, cols, rows, inv_cell)].push(i as u32);
    }
    grid
}

fn process_cell(
    pos: &mut [Vec2],
    vel: &mut [Vec2],
    rad: &[f32],
    grid: &[Vec<u32>],
    idx: usize,
    cx: i32,
    cy: i32,
    cols: i32,
    rows: i32,
) {
    let cell = grid[idx].clone();
    for a in 0..cell.len() {
        for b in (a + 1)..cell.len() {
            resolve_pair(cell[a] as usize, cell[b] as usize, pos, vel, rad);
        }
    }
    for &(dx, dy) in &[(1i32, -1i32), (1, 0), (1, 1), (0, 1)] {
        let (nx, ny) = (cx + dx, cy + dy);
        if nx < 0 || ny < 0 || nx >= cols || ny >= rows {
            continue;
        }
        let nidx = (nx + ny * cols) as usize;
        for &i in &cell {
            for &j in &grid[nidx] {
                resolve_pair(i as usize, j as usize, pos, vel, rad);
            }
        }
    }
}

fn resolve_particles(pos: &mut [Vec2], vel: &mut [Vec2], rad: &[f32]) {
    if pos.len() < 2 {
        return;
    }
    let max_r = rad.iter().cloned().fold(0.0f32, f32::max);
    let cell = (max_r * 2.0 + 1.0).max(8.0);
    let inv = 1.0 / cell;
    let (cols, rows) = (
        ((HALF_WIDTH * 2.0) * inv).ceil() as i32 + 2,
        ((HALF_HEIGHT * 2.0) * inv).ceil() as i32 + 2,
    );
    let grid = build_grid(pos, cols, rows, inv);
    for cy in 0..rows {
        for cx in 0..cols {
            let idx = (cx + cy * cols) as usize;
            if !grid[idx].is_empty() {
                process_cell(pos, vel, rad, &grid, idx, cx, cy, cols, rows);
            }
        }
    }
}

#[inline]
fn resolve_pair(i: usize, j: usize, pos: &mut [Vec2], vel: &mut [Vec2], rad: &[f32]) {
    let delta = pos[j] - pos[i];
    let r_sum = rad[i] + rad[j];
    let dist_sq = delta.length_squared();
    if dist_sq >= r_sum * r_sum || dist_sq < 1e-12 {
        return;
    }
    let dist = dist_sq.sqrt();
    let n = delta / dist;
    let half = (r_sum - dist) * 0.5;
    pos[i] -= n * half;
    pos[j] += n * half;
    let vn = (vel[j] - vel[i]).dot(n);
    if vn < 0.0 {
        let imp = n * (1.0 + PARTICLE_RESTITUTION) * vn * 0.5;
        vel[i] += imp;
        vel[j] -= imp;
    }
}
