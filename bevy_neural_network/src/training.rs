//! Ressources d'état et machine d'entraînement pas-à-pas.
//!
//! Un « cycle » d'apprentissage correspond à un pas de descente de gradient
//! stochastique (SGD) sur l'échantillon actif, décomposé en trois phases que
//! l'on peut jouer pas à pas :
//!   * `Forward`  — propagation avant, l'onde traverse les couches de gauche à droite ;
//!   * `Backward` — rétropropagation, l'onde des gradients remonte de droite à gauche ;
//!   * `Update`   — application du gradient, les poids changent, la loss est enregistrée.

use bevy::prelude::*;

use crate::config::STEPS_PER_TURBO;
use crate::dataset::{Dataset, DatasetKind};
use crate::network::{Network, OutAct};

#[derive(Resource)]
pub struct Net(pub Network);

#[derive(Resource)]
pub struct Data(pub Dataset);

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Forward,
    Backward,
    Update,
}

impl Phase {
    pub fn label(self) -> &'static str {
        match self {
            Phase::Forward => "AVANT  (forward)",
            Phase::Backward => "ARRIERE (backward)",
            Phase::Update => "MAJ des poids",
        }
    }
}

/// État de l'animation et de la machine à phases.
#[derive(Resource)]
pub struct Anim {
    pub phase: Phase,
    /// Segment (transition entre deux couches) en cours d'animation.
    pub seg: i32,
    /// Progression dans le segment courant, dans [0, 1].
    pub t: f32,
    /// Lecture continue (play/pause).
    pub running: bool,
    /// Demande d'avancer d'une seule phase puis de s'arrêter.
    pub step_request: bool,
    /// Durée d'un segment, en secondes (contrôle la vitesse).
    pub seg_dur: f32,
    /// Indice de l'échantillon en cours d'illustration.
    pub active: usize,
    /// Le calcul de la phase courante a-t-il déjà été exécuté ?
    pub entered: bool,
}

impl Default for Anim {
    fn default() -> Self {
        Anim {
            phase: Phase::Forward,
            seg: 0,
            t: 0.0,
            running: false,
            step_request: false,
            seg_dur: 0.45,
            active: 0,
            entered: false,
        }
    }
}

/// Historique de la loss (une valeur par pas SGD).
#[derive(Resource, Default)]
pub struct LossHist(pub Vec<f32>);

#[derive(Resource)]
pub struct Hyper {
    pub lr: f32,
}

impl Default for Hyper {
    fn default() -> Self {
        Hyper { lr: 0.5 }
    }
}

/// Topologie modifiable : tailles des couches *cachées* uniquement.
#[derive(Resource)]
pub struct Topology {
    pub hidden: Vec<usize>,
    /// Couche cachée sélectionnée pour ajouter/retirer des neurones.
    pub sel: usize,
    /// Faut-il reconstruire le réseau ? (changement de topologie ou de dataset)
    pub dirty: bool,
}

impl Default for Topology {
    fn default() -> Self {
        Topology {
            hidden: vec![5, 5],
            sel: 0,
            dirty: false,
        }
    }
}

/// Mode entraînement rapide (sans animation), activé tant qu'une touche est tenue.
#[derive(Resource, Default)]
pub struct Turbo(pub bool);

/// Nombre total de pas SGD effectués depuis la dernière réinitialisation.
#[derive(Resource, Default)]
pub struct Steps(pub usize);

/// Drapeau : les entités neurones doivent être reconstruites.
#[derive(Resource, Default)]
pub struct GraphDirty(pub bool);

const LOSS_CAP: usize = 4000;

fn push_loss(loss: &mut LossHist, v: f32) {
    loss.0.push(v);
    if loss.0.len() > LOSS_CAP {
        let excess = loss.0.len() - LOSS_CAP;
        loss.0.drain(0..excess);
    }
}

/// Loss MSE moyenne sur tout le jeu de données (sans perturber l'état stocké).
pub fn dataset_loss(net: &Network, data: &Dataset) -> f32 {
    if data.samples.is_empty() {
        return 0.0;
    }
    let mut s = 0.0;
    for smp in &data.samples {
        let o = net.forward_pure(&smp.x);
        for j in 0..o.len() {
            let e = o[j] - smp.y.get(j).copied().unwrap_or(0.0);
            s += 0.5 * e * e;
        }
    }
    s / data.samples.len() as f32
}

/// Reconstruit le réseau quand la topologie ou le dataset change.
pub fn rebuild_network(
    mut topo: ResMut<Topology>,
    mut net: ResMut<Net>,
    data: Res<Data>,
    mut anim: ResMut<Anim>,
    mut loss: ResMut<LossHist>,
    mut steps: ResMut<Steps>,
    mut graph_dirty: ResMut<GraphDirty>,
) {
    if !topo.dirty {
        return;
    }
    topo.dirty = false;
    if topo.sel >= topo.hidden.len() {
        topo.sel = topo.hidden.len().saturating_sub(1);
    }

    let mut sizes = vec![data.0.in_dim];
    sizes.extend(topo.hidden.iter().copied());
    sizes.push(data.0.out_dim);

    let out = if data.0.is_regression() {
        OutAct::Linear
    } else {
        OutAct::Sigmoid
    };
    net.0 = Network::new(&sizes, out);
    *anim = Anim::default();
    loss.0.clear();
    steps.0 = 0;
    graph_dirty.0 = true;
}

/// Cœur de la boucle : fait avancer l'entraînement et l'animation.
pub fn advance_training(
    time: Res<Time>,
    mut anim: ResMut<Anim>,
    mut net: ResMut<Net>,
    data: Res<Data>,
    hyper: Res<Hyper>,
    mut loss: ResMut<LossHist>,
    mut steps: ResMut<Steps>,
    turbo: Res<Turbo>,
) {
    let n = data.0.samples.len();
    if n == 0 {
        return;
    }

    // Mode turbo : on enchaîne les pas SGD sans animation pour converger vite.
    if turbo.0 {
        for _ in 0..STEPS_PER_TURBO {
            let s = &data.0.samples[anim.active % n];
            net.0.forward(&s.x);
            net.0.backward(&s.y);
            net.0.apply_grad(hyper.lr);
            anim.active = (anim.active + 1) % n;
            steps.0 += 1;
        }
        let lv = dataset_loss(&net.0, &data.0);
        push_loss(&mut loss, lv);
        // On laisse un forward propre de l'échantillon courant pour l'affichage.
        let s = &data.0.samples[anim.active % n];
        net.0.forward(&s.x);
        anim.phase = Phase::Forward;
        anim.seg = 0;
        anim.t = 0.0;
        anim.entered = true;
        return;
    }

    // Exécute une fois le calcul de la phase courante (au moment où on y entre).
    if !anim.entered {
        let active = anim.active % n;
        match anim.phase {
            Phase::Forward => {
                let s = &data.0.samples[active];
                net.0.forward(&s.x);
            }
            Phase::Backward => {
                let s = &data.0.samples[active];
                net.0.backward(&s.y);
            }
            Phase::Update => {
                net.0.apply_grad(hyper.lr);
                steps.0 += 1;
                let lv = dataset_loss(&net.0, &data.0);
                push_loss(&mut loss, lv);
            }
        }
        anim.entered = true;
    }

    let advancing = anim.running || anim.step_request;
    if !advancing {
        return;
    }

    let l = net.0.sizes.len();
    let seg_count = match anim.phase {
        Phase::Forward | Phase::Backward => l.saturating_sub(1) as i32,
        Phase::Update => 1,
    };

    anim.t += time.delta_secs() / anim.seg_dur.max(0.01);
    if anim.t >= 1.0 {
        anim.t = 0.0;
        anim.seg += 1;
        if anim.seg >= seg_count {
            anim.seg = 0;
            let was_stepping = anim.step_request;
            match anim.phase {
                Phase::Forward => anim.phase = Phase::Backward,
                Phase::Backward => anim.phase = Phase::Update,
                Phase::Update => {
                    anim.phase = Phase::Forward;
                    anim.active = (anim.active + 1) % n;
                }
            }
            anim.entered = false;
            // En mode pas-à-pas, on s'arrête à la frontière de phase.
            if was_stepping {
                anim.step_request = false;
            }
        }
    }
}

/// Petit utilitaire pour basculer de jeu de données depuis l'entrée clavier.
pub fn switch_dataset(data: &mut Data, topo: &mut Topology, kind: DatasetKind) {
    if data.0.kind == kind {
        return;
    }
    data.0 = Dataset::new(kind);
    topo.dirty = true;
}
