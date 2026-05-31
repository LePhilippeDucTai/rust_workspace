//! Percolation de sites sur une grille carrée, visualisée avec Bevy.
//!
//! Chaque cellule de la grille est « ouverte » avec une probabilité `p` et
//! « fermée » sinon. On cherche ensuite l'amas (cluster) de cellules ouvertes
//! connectées à la rangée du haut. Si cet amas atteint la rangée du bas, le
//! système *percole* : il existe un chemin continu de haut en bas.
//!
//! L'intérêt probabiliste est la transition de phase brutale : en dessous du
//! seuil critique `p_c ≈ 0.5927` (percolation de sites sur réseau carré), la
//! percolation est quasi impossible ; au-dessus, elle devient quasi certaine.
//! Faites varier `p` autour de cette valeur pour voir le phénomène.
//!
//! Contrôles :
//!   - Flèches Haut/Bas  : p ± 0.01
//!   - Flèches Droite/Gauche : p ± 0.05
//!   - Espace / R        : nouveau tirage aléatoire (même p)
//!   - A                 : balayage automatique de p (on/off)

use bevy::prelude::*;

const GRID_W: usize = 100;
const GRID_H: usize = 100;
const CELL_SIZE: f32 = 7.0;
const GAP: f32 = 0.0;

/// Petit générateur pseudo-aléatoire (xorshift64*), pour éviter toute
/// dépendance externe et rester déterministe/seedable.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        // Évite l'état nul qui resterait bloqué.
        Rng(seed | 1)
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Flottant uniforme dans [0, 1).
    fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u32 << 24) as f32
    }
}

#[derive(Resource)]
struct Sim {
    p: f32,
    /// `true` si la cellule est ouverte.
    open: Vec<bool>,
    /// `true` si la cellule ouverte est connectée à la rangée du haut.
    top_connected: Vec<bool>,
    percolates: bool,
    largest_cluster: usize,
    rng: Rng,
    auto: bool,
    auto_dir: f32,
    dirty: bool,
}

impl Sim {
    fn idx(x: usize, y: usize) -> usize {
        y * GRID_W + x
    }

    /// Effectue un nouveau tirage aléatoire des cellules ouvertes.
    fn resample(&mut self) {
        let p = self.p;
        for cell in self.open.iter_mut() {
            *cell = self.rng.next_f32() < p;
        }
        self.recompute_clusters();
        self.dirty = true;
    }

    /// Recalcule, par parcours en largeur depuis la rangée du haut, l'ensemble
    /// des cellules ouvertes connectées au sommet, et détermine la percolation.
    fn recompute_clusters(&mut self) {
        let n = GRID_W * GRID_H;
        for c in self.top_connected.iter_mut() {
            *c = false;
        }

        // BFS multi-sources depuis toutes les cellules ouvertes du haut.
        let mut stack: Vec<usize> = Vec::with_capacity(n / 4);
        for x in 0..GRID_W {
            let i = Self::idx(x, 0);
            if self.open[i] && !self.top_connected[i] {
                self.top_connected[i] = true;
                stack.push(i);
            }
        }

        let mut connected_count = 0usize;
        let mut percolates = false;
        while let Some(i) = stack.pop() {
            connected_count += 1;
            let x = i % GRID_W;
            let y = i / GRID_W;
            if y == GRID_H - 1 {
                percolates = true;
            }
            // 4-voisinage
            let mut push_if = |nx: usize, ny: usize, sim: &mut Sim, st: &mut Vec<usize>| {
                let ni = Self::idx(nx, ny);
                if sim.open[ni] && !sim.top_connected[ni] {
                    sim.top_connected[ni] = true;
                    st.push(ni);
                }
            };
            if x > 0 {
                push_if(x - 1, y, self, &mut stack);
            }
            if x + 1 < GRID_W {
                push_if(x + 1, y, self, &mut stack);
            }
            if y > 0 {
                push_if(x, y - 1, self, &mut stack);
            }
            if y + 1 < GRID_H {
                push_if(x, y + 1, self, &mut stack);
            }
        }

        self.percolates = percolates;
        self.largest_cluster = connected_count;
    }

    fn log_state(&self) {
        info!(
            "p = {:.3}  | amas relié au haut = {} cellules | percolation : {}",
            self.p,
            self.largest_cluster,
            if self.percolates { "OUI" } else { "non" }
        );
    }
}

/// Repère la position d'un sprite dans la grille.
#[derive(Component)]
struct Cell {
    x: usize,
    y: usize,
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Percolation de sites — Bevy".to_string(),
                resolution: (
                    GRID_W as f32 * (CELL_SIZE + GAP) + 40.0,
                    GRID_H as f32 * (CELL_SIZE + GAP) + 40.0,
                )
                    .into(),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::srgb(0.05, 0.05, 0.08)))
        .insert_resource(init_sim())
        .add_systems(Startup, setup)
        .add_systems(Update, (handle_input, auto_sweep, redraw).chain())
        .run();
}

fn init_sim() -> Sim {
    let mut sim = Sim {
        p: 0.55,
        open: vec![false; GRID_W * GRID_H],
        top_connected: vec![false; GRID_W * GRID_H],
        percolates: false,
        largest_cluster: 0,
        rng: Rng::new(0x9E37_79B9_7F4A_7C15),
        auto: false,
        auto_dir: 1.0,
        dirty: true,
    };
    sim.resample();
    sim.log_state();
    sim
}

fn setup(mut commands: Commands, sim: Res<Sim>) {
    commands.spawn(Camera2d);

    let total_w = GRID_W as f32 * (CELL_SIZE + GAP);
    let total_h = GRID_H as f32 * (CELL_SIZE + GAP);
    let origin_x = -total_w / 2.0 + CELL_SIZE / 2.0;
    let origin_y = -total_h / 2.0 + CELL_SIZE / 2.0;

    for y in 0..GRID_H {
        for x in 0..GRID_W {
            let px = origin_x + x as f32 * (CELL_SIZE + GAP);
            let py = origin_y + y as f32 * (CELL_SIZE + GAP);
            commands.spawn((
                Sprite {
                    color: cell_color(&sim, x, y),
                    custom_size: Some(Vec2::splat(CELL_SIZE)),
                    ..default()
                },
                Transform::from_xyz(px, py, 0.0),
                Cell { x, y },
            ));
        }
    }
}

/// Couleur d'une cellule selon son état.
fn cell_color(sim: &Sim, x: usize, y: usize) -> Color {
    let i = Sim::idx(x, y);
    if !sim.open[i] {
        // Cellule fermée : presque noire.
        Color::srgb(0.10, 0.10, 0.13)
    } else if sim.top_connected[i] {
        if sim.percolates {
            // Amas percolant : rouge vif.
            Color::srgb(0.95, 0.25, 0.20)
        } else {
            // Relié au haut mais sans traverser : orange.
            Color::srgb(0.95, 0.65, 0.20)
        }
    } else {
        // Ouverte mais isolée du haut : bleu acier.
        Color::srgb(0.30, 0.55, 0.80)
    }
}

fn handle_input(keys: Res<ButtonInput<KeyCode>>, mut sim: ResMut<Sim>) {
    let mut changed_p = false;

    if keys.just_pressed(KeyCode::ArrowUp) {
        sim.p = (sim.p + 0.01).min(1.0);
        changed_p = true;
    }
    if keys.just_pressed(KeyCode::ArrowDown) {
        sim.p = (sim.p - 0.01).max(0.0);
        changed_p = true;
    }
    if keys.just_pressed(KeyCode::ArrowRight) {
        sim.p = (sim.p + 0.05).min(1.0);
        changed_p = true;
    }
    if keys.just_pressed(KeyCode::ArrowLeft) {
        sim.p = (sim.p - 0.05).max(0.0);
        changed_p = true;
    }
    if keys.just_pressed(KeyCode::KeyA) {
        sim.auto = !sim.auto;
        info!("Balayage automatique : {}", if sim.auto { "ON" } else { "OFF" });
    }

    let new_sample = keys.just_pressed(KeyCode::Space) || keys.just_pressed(KeyCode::KeyR);

    if changed_p || new_sample {
        sim.resample();
        sim.log_state();
    }
}

/// Fait osciller `p` lentement entre 0.40 et 0.75 quand le mode auto est actif,
/// pour visualiser la transition de phase en continu.
fn auto_sweep(time: Res<Time>, mut sim: ResMut<Sim>) {
    if !sim.auto {
        return;
    }
    let step = 0.06 * time.delta_secs() * sim.auto_dir;
    sim.p += step;
    if sim.p >= 0.75 {
        sim.p = 0.75;
        sim.auto_dir = -1.0;
    } else if sim.p <= 0.40 {
        sim.p = 0.40;
        sim.auto_dir = 1.0;
    }
    sim.resample();
}

/// Met à jour la couleur des sprites lorsque la simulation a changé.
fn redraw(mut sim: ResMut<Sim>, mut query: Query<(&Cell, &mut Sprite)>) {
    if !sim.dirty {
        return;
    }
    for (cell, mut sprite) in &mut query {
        sprite.color = cell_color(&sim, cell.x, cell.y);
    }
    sim.dirty = false;
}
