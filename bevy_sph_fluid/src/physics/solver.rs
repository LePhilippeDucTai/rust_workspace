//! Solveur SPH (*Smoothed Particle Hydrodynamics*) — Müller, Charypar &
//! Gross 2003, « Particle-Based Fluid Simulation for Interactive
//! Applications ».
//!
//! **Découplé de l'ECS** : il opère sur de simples `Vec<Vec2>`, ce qui le
//! rend testable sans démarrer Bevy (cf. tests de stabilité en fin de
//! fichier). La recherche de voisins utilise une **grille uniforme** dont la
//! cellule vaut le rayon de lissage `H` : chaque particule ne consulte que
//! les 3×3 cellules autour d'elle.
//!
//! Chaque pas suit le schéma classique :
//! 1. densité     ρᵢ = Σⱼ m Wₚₒₗᵧ₆(rᵢⱼ)
//! 2. pression    pᵢ = max(0, k (ρᵢ − ρ₀))           (clampée ⇒ pas de cohésion parasite)
//! 3. forces      pression (noyau spiky) + viscosité (laplacien) + gravité
//! 4. intégration Euler semi-implicite, puis contraintes de parois.

use bevy::prelude::Vec2;
use std::f32::consts::PI;

use crate::config::{
    BOUND_DAMPING, BOX_BOTTOM, BOX_LEFT, BOX_RIGHT, BOX_TOP, GAS_CONST, H, MASS, MOUSE_RADIUS,
};

/// Vitesse maximale autorisée (px/s) : borne anti-explosion, indispensable
/// avec une EOS raide intégrée en Euler explicite (les pics de pression dans
/// les zones très comprimées génèreraient sinon des vitesses divergentes).
const MAX_SPEED: f32 = 1200.0;

/// Force radiale appliquée par l'utilisateur (curseur). `strength > 0`
/// repousse, `strength < 0` attire.
#[derive(Clone, Copy)]
pub struct ExternalForce {
    pub center: Vec2,
    pub strength: f32,
}

pub struct Fluid {
    pub pos: Vec<Vec2>,
    pub vel: Vec<Vec2>,
    pub density: Vec<f32>,
    pub pressure: Vec<f32>,
    /// Densité de référence ρ₀, calibrée sur la configuration initiale.
    pub rest_density: f32,
    /// Raideur de l'équation d'état (initialisée depuis `config::GAS_CONST`).
    pub gas_const: f32,

    // Coefficients de normalisation des noyaux 2D (précalculés).
    hsq: f32,
    poly6: f32,
    spiky_grad: f32,
    visc_lap: f32,

    // Grille uniforme de recherche de voisins.
    nx: usize,
    ny: usize,
    cells: Vec<Vec<u32>>,

    // Accumulateur d'accélérations, réutilisé entre les pas.
    accel: Vec<Vec2>,
}

impl Fluid {
    pub fn new() -> Self {
        let nx = ((BOX_RIGHT - BOX_LEFT) / H).ceil() as usize + 1;
        let ny = ((BOX_TOP - BOX_BOTTOM) / H).ceil() as usize + 1;
        Self {
            pos: Vec::new(),
            vel: Vec::new(),
            density: Vec::new(),
            pressure: Vec::new(),
            rest_density: 0.0,
            gas_const: GAS_CONST,
            hsq: H * H,
            // Noyaux 2D normalisés sur le disque de rayon H.
            poly6: 4.0 / (PI * H.powi(8)),
            spiky_grad: 30.0 / (PI * H.powi(5)),
            visc_lap: 40.0 / (PI * H.powi(5)),
            nx,
            ny,
            cells: (0..nx * ny).map(|_| Vec::new()).collect(),
            accel: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.pos.len()
    }

    pub fn add_particle(&mut self, pos: Vec2) {
        self.pos.push(pos);
        self.vel.push(Vec2::ZERO);
        self.density.push(0.0);
        self.pressure.push(0.0);
        self.accel.push(Vec2::ZERO);
    }

    /// Calibre ρ₀ sur la disposition courante : on prend la densité **maximale**
    /// (particule de cœur, voisinage complet). Au repos, le cœur a donc une
    /// pression nulle, et seules les zones comprimées génèrent une force ⇒
    /// démarrage sans explosion.
    pub fn calibrate_rest_density(&mut self) {
        build_grid(&mut self.cells, &self.pos, self.nx, self.ny);
        compute_density(
            &mut self.density,
            &self.pos,
            &self.cells,
            self.nx,
            self.ny,
            self.poly6,
            self.hsq,
        );
        self.rest_density = self
            .density
            .iter()
            .copied()
            .fold(0.0_f32, f32::max)
            .max(f32::MIN_POSITIVE);
    }

    /// Avance la simulation d'un pas `dt`.
    pub fn step(&mut self, dt: f32, gravity: Vec2, viscosity: f32, external: Option<ExternalForce>) {
        if self.pos.is_empty() {
            return;
        }
        // Sécurité : si jamais non calibré, le faire à la volée.
        if self.rest_density <= 0.0 {
            self.calibrate_rest_density();
        }

        // Emprunts disjoints sur chaque champ pour contenter le borrow-checker.
        let Fluid {
            pos,
            vel,
            density,
            pressure,
            rest_density,
            gas_const,
            hsq,
            poly6,
            spiky_grad,
            visc_lap,
            nx,
            ny,
            cells,
            accel,
        } = self;

        build_grid(cells, pos, *nx, *ny);
        compute_density(density, pos, cells, *nx, *ny, *poly6, *hsq);

        // Équation d'état, pression clampée à zéro (anti tensile-instability).
        for i in 0..density.len() {
            pressure[i] = (*gas_const * (density[i] - *rest_density)).max(0.0);
        }

        compute_accel(
            accel, pos, vel, density, pressure, cells, *nx, *ny, *spiky_grad, *visc_lap, viscosity,
            gravity, external,
        );

        // Euler semi-implicite + clamp de vitesse + contraintes de parois.
        let max_speed_sq = MAX_SPEED * MAX_SPEED;
        for i in 0..pos.len() {
            let mut v = vel[i] + accel[i] * dt;
            let speed_sq = v.length_squared();
            if speed_sq > max_speed_sq {
                v *= MAX_SPEED / speed_sq.sqrt();
            }
            vel[i] = v;
            pos[i] += v * dt;
            enforce_bounds(&mut pos[i], &mut vel[i]);
        }
    }
}

impl Default for Fluid {
    fn default() -> Self {
        Self::new()
    }
}

// ───────────────────────── Grille uniforme ─────────────────────────

#[inline]
fn cell_coords(p: Vec2, nx: usize, ny: usize) -> (usize, usize) {
    let cx = (((p.x - BOX_LEFT) / H) as isize).clamp(0, nx as isize - 1) as usize;
    let cy = (((p.y - BOX_BOTTOM) / H) as isize).clamp(0, ny as isize - 1) as usize;
    (cx, cy)
}

fn build_grid(cells: &mut [Vec<u32>], pos: &[Vec2], nx: usize, ny: usize) {
    for cell in cells.iter_mut() {
        cell.clear();
    }
    for (i, &p) in pos.iter().enumerate() {
        let (cx, cy) = cell_coords(p, nx, ny);
        cells[cx + cy * nx].push(i as u32);
    }
}

/// Applique `f(j)` à chaque voisin candidat `j` de la particule en `p`
/// (toutes les particules des 3×3 cellules autour, `j == i` inclus).
#[inline]
fn for_each_neighbor(p: Vec2, cells: &[Vec<u32>], nx: usize, ny: usize, mut f: impl FnMut(usize)) {
    let (cx, cy) = cell_coords(p, nx, ny);
    for dy in -1..=1_isize {
        let ny_ = cy as isize + dy;
        if ny_ < 0 || ny_ >= ny as isize {
            continue;
        }
        for dx in -1..=1_isize {
            let nx_ = cx as isize + dx;
            if nx_ < 0 || nx_ >= nx as isize {
                continue;
            }
            for &j in &cells[nx_ as usize + ny_ as usize * nx] {
                f(j as usize);
            }
        }
    }
}

// ───────────────────── Densité, pression, forces ─────────────────────

fn compute_density(
    density: &mut [f32],
    pos: &[Vec2],
    cells: &[Vec<u32>],
    nx: usize,
    ny: usize,
    poly6: f32,
    hsq: f32,
) {
    for i in 0..pos.len() {
        let pi = pos[i];
        let mut rho = 0.0;
        for_each_neighbor(pi, cells, nx, ny, |j| {
            let r2 = (pos[j] - pi).length_squared();
            if r2 < hsq {
                let d = hsq - r2;
                rho += MASS * poly6 * d * d * d;
            }
        });
        density[i] = rho;
    }
}

#[allow(clippy::too_many_arguments)]
fn compute_accel(
    accel: &mut [Vec2],
    pos: &[Vec2],
    vel: &[Vec2],
    density: &[f32],
    pressure: &[f32],
    cells: &[Vec<u32>],
    nx: usize,
    ny: usize,
    spiky_grad: f32,
    visc_lap: f32,
    viscosity: f32,
    gravity: Vec2,
    external: Option<ExternalForce>,
) {
    for i in 0..pos.len() {
        let pi = pos[i];
        let vi = vel[i];
        let mut f_press = Vec2::ZERO;
        let mut f_visc = Vec2::ZERO;

        for_each_neighbor(pi, cells, nx, ny, |j| {
            if j == i {
                return;
            }
            let rij = pi - pos[j]; // de j vers i
            let r = rij.length();
            if r < H && r > 1e-6 {
                let dir = rij / r;
                let hr = H - r;
                // Force de pression (gradient du noyau spiky), symétrisée.
                f_press +=
                    dir * (MASS * (pressure[i] + pressure[j]) / (2.0 * density[j]) * spiky_grad
                        * hr
                        * hr);
                // Force de viscosité (laplacien du noyau de viscosité).
                f_visc += (vel[j] - vi) * (viscosity * MASS / density[j] * visc_lap * hr);
            }
        });

        // a = (F_pression + F_viscosité) / ρ + g  (+ forçage utilisateur)
        let mut a = (f_press + f_visc) / density[i] + gravity;

        if let Some(ext) = external {
            let d = pi - ext.center;
            let dist = d.length();
            if dist < MOUSE_RADIUS && dist > 1e-6 {
                let falloff = 1.0 - dist / MOUSE_RADIUS;
                a += (d / dist) * (ext.strength * falloff);
            }
        }

        accel[i] = a;
    }
}

#[inline]
fn enforce_bounds(p: &mut Vec2, v: &mut Vec2) {
    if p.x < BOX_LEFT {
        p.x = BOX_LEFT;
        v.x *= BOUND_DAMPING;
    } else if p.x > BOX_RIGHT {
        p.x = BOX_RIGHT;
        v.x *= BOUND_DAMPING;
    }
    if p.y < BOX_BOTTOM {
        p.y = BOX_BOTTOM;
        v.y *= BOUND_DAMPING;
    } else if p.y > BOX_TOP {
        p.y = BOX_TOP;
        v.y *= BOUND_DAMPING;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{DT, GRAVITY, SPACING, VISCOSITY};

    /// Construit un petit bloc carré de `n × n` particules centré près du bas.
    fn block(n: usize) -> Fluid {
        let mut fluid = Fluid::new();
        let x0 = -((n as f32) * SPACING) * 0.5;
        for row in 0..n {
            for col in 0..n {
                let x = x0 + col as f32 * SPACING;
                let y = BOX_BOTTOM + 60.0 + row as f32 * SPACING;
                fluid.add_particle(Vec2::new(x, y));
            }
        }
        fluid
    }

    /// Colonne haute (10×24) servant aux diagnostics de tassement.
    fn tall_column() -> Fluid {
        let mut fluid = Fluid::new();
        for row in 0..24 {
            for col in 0..10 {
                let x = BOX_LEFT + 60.0 + col as f32 * SPACING;
                let y = BOX_BOTTOM + 40.0 + row as f32 * SPACING;
                fluid.add_particle(Vec2::new(x, y));
            }
        }
        fluid.calibrate_rest_density();
        fluid
    }

    fn height_of(fluid: &Fluid) -> f32 {
        let (mut miny, mut maxy) = (f32::MAX, f32::MIN);
        for p in &fluid.pos {
            miny = miny.min(p.y);
            maxy = maxy.max(p.y);
        }
        maxy - miny
    }

    #[test]
    #[ignore]
    fn diag_app_scale() {
        use crate::config::{INIT_COLUMNS, INIT_ROWS};
        let g = Vec2::new(0.0, GRAVITY);
        for &gas in &[crate::config::GAS_CONST] {
            let mut fluid = Fluid::new();
            for row in 0..INIT_ROWS {
                for col in 0..INIT_COLUMNS {
                    let x = BOX_LEFT + 50.0 + col as f32 * SPACING;
                    let y = BOX_BOTTOM + 50.0 + row as f32 * SPACING;
                    fluid.add_particle(Vec2::new(x, y));
                }
            }
            fluid.calibrate_rest_density();
            fluid.gas_const = gas;
            for _ in 0..20_000 {
                fluid.step(DT, g, VISCOSITY, None);
            }
            let (mut miny, mut maxy) = (f32::MAX, f32::MIN);
            for p in &fluid.pos {
                miny = miny.min(p.y);
                maxy = maxy.max(p.y);
            }
            let mean_d = fluid.density.iter().sum::<f32>() / fluid.len() as f32 / fluid.rest_density;
            let v_max = fluid.vel.iter().map(|v| v.length()).fold(0.0_f32, f32::max);
            let mut bands = [0u32; 10];
            for p in &fluid.pos {
                let b = (((p.y - miny) / 20.0) as usize).min(bands.len() - 1);
                bands[b] += 1;
            }
            eprintln!(
                "gas={gas:>8.0} hauteur={:>4.0}px ρ_moy/ρ₀={mean_d:.2} v_max={v_max:.0} profil(bas→haut/20px)={bands:?}",
                maxy - miny
            );
        }
    }

    #[test]
    #[ignore]
    fn diag_gas_const_sweep() {
        let g = Vec2::new(0.0, GRAVITY);
        eprintln!("colonne initiale : hauteur = {:.0}px", height_of(&tall_column()));
        for &gas in &[3_000.0_f32, 10_000.0, 30_000.0, 100_000.0, 300_000.0] {
            let mut fluid = tall_column();
            fluid.gas_const = gas;
            for _ in 0..15_000 {
                fluid.step(DT, g, VISCOSITY, None);
            }
            let mean_d = fluid.density.iter().sum::<f32>() / fluid.len() as f32 / fluid.rest_density;
            let max_speed = fluid.vel.iter().map(|v| v.length()).fold(0.0_f32, f32::max);
            eprintln!(
                "gas={gas:>8.0}  hauteur={:>5.0}px  ρ_moy/ρ₀={mean_d:>5.2}  v_max={max_speed:>6.0}",
                height_of(&fluid)
            );
        }
    }

    #[test]
    fn test_density_is_positive_after_calibration() {
        let mut fluid = block(8);
        fluid.calibrate_rest_density();
        assert!(fluid.rest_density > 0.0);
        // Toute particule possède au moins sa propre contribution.
        assert!(fluid.density.iter().all(|&d| d > 0.0));
    }

    #[test]
    fn test_rest_density_is_the_maximum() {
        let mut fluid = block(8);
        fluid.calibrate_rest_density();
        let max = fluid.density.iter().copied().fold(0.0_f32, f32::max);
        assert!((fluid.rest_density - max).abs() < 1e-6);
    }

    #[test]
    fn test_empty_fluid_step_is_noop() {
        let mut fluid = Fluid::new();
        fluid.step(DT, Vec2::new(0.0, GRAVITY), VISCOSITY, None);
        assert_eq!(fluid.len(), 0);
    }

    /// Test de **stabilité numérique** : on laisse un bloc retomber et se
    /// stabiliser, puis on vérifie l'absence de NaN/Inf, le confinement dans
    /// la boîte, et l'incompressibilité du volume.
    ///
    /// L'indicateur d'incompressibilité retenu est la densité **moyenne** : en
    /// WCSPH, la couche limite plaquée contre le fond se comprime toujours plus
    /// que le volume, donc le max ponctuel n'est pas représentatif. On borne
    /// tout de même le max pour détecter une vraie divergence.
    #[test]
    fn test_simulation_stays_stable_and_bounded() {
        let mut fluid = block(12);
        fluid.calibrate_rest_density();
        let g = Vec2::new(0.0, GRAVITY);

        for _ in 0..8000 {
            fluid.step(DT, g, VISCOSITY, None);
        }

        for (k, p) in fluid.pos.iter().enumerate() {
            assert!(p.x.is_finite() && p.y.is_finite(), "NaN/Inf à l'indice {k}");
            assert!(
                p.x >= BOX_LEFT - 1.0 && p.x <= BOX_RIGHT + 1.0,
                "particule {k} hors boîte en x : {}",
                p.x
            );
            assert!(
                p.y >= BOX_BOTTOM - 1.0 && p.y <= BOX_TOP + 1.0,
                "particule {k} hors boîte en y : {}",
                p.y
            );
        }

        let n = fluid.len() as f32;
        let mean_density = fluid.density.iter().sum::<f32>() / n;
        let max_density = fluid.density.iter().copied().fold(0.0_f32, f32::max);
        let rho0 = fluid.rest_density;
        eprintln!(
            "ρ₀ = {rho0:.3}   ρ_moy = {mean_density:.3} ({:.2}×)   ρ_max = {max_density:.3} ({:.2}×)",
            mean_density / rho0,
            max_density / rho0
        );
        assert!(
            mean_density < 1.4 * rho0,
            "volume trop comprimé : ρ_moy = {mean_density}, ρ₀ = {rho0}"
        );
        assert!(
            max_density < 3.5 * rho0,
            "divergence locale : ρ_max = {max_density}, ρ₀ = {rho0}"
        );
    }

    /// Après stabilisation, le fluide doit s'être déposé : la hauteur moyenne
    /// est nettement sous le sommet de la boîte (il ne flotte pas / n'explose
    /// pas vers le haut).
    #[test]
    fn test_fluid_settles_at_the_bottom() {
        let mut fluid = block(12);
        fluid.calibrate_rest_density();
        let g = Vec2::new(0.0, GRAVITY);
        for _ in 0..8000 {
            fluid.step(DT, g, VISCOSITY, None);
        }
        let mean_y = fluid.pos.iter().map(|p| p.y).sum::<f32>() / fluid.len() as f32;
        assert!(
            mean_y < (BOX_TOP + BOX_BOTTOM) * 0.5,
            "le fluide ne s'est pas déposé : y moyen = {mean_y}"
        );
    }
}
