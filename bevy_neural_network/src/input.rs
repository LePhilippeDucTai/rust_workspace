//! Contrôles clavier : lecture, pas-à-pas, topologie, datasets, hyperparamètres.

use bevy::prelude::*;

use crate::dataset::DatasetKind;
use crate::training::{Anim, Data, Hyper, Topology, Turbo, switch_dataset};

const MAX_NODES: usize = 12;
const MAX_DEPTH: usize = 6;

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, handle_input);
    }
}

fn handle_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut anim: ResMut<Anim>,
    mut topo: ResMut<Topology>,
    mut hyper: ResMut<Hyper>,
    mut turbo: ResMut<Turbo>,
    mut data: ResMut<Data>,
) {
    // Lecture / pause / pas-à-pas / reset.
    if keys.just_pressed(KeyCode::KeyP) {
        anim.running = !anim.running;
    }
    if keys.just_pressed(KeyCode::Space) {
        anim.running = false;
        anim.step_request = true;
    }
    if keys.just_pressed(KeyCode::KeyR) {
        topo.dirty = true; // réinitialise poids, loss et compteurs
    }

    // Entraînement rapide tant que F est tenu.
    turbo.0 = keys.pressed(KeyCode::KeyF);

    // Sélection de la couche cachée à éditer.
    if keys.just_pressed(KeyCode::ArrowLeft) && topo.sel > 0 {
        topo.sel -= 1;
    }
    if keys.just_pressed(KeyCode::ArrowRight) && topo.sel + 1 < topo.hidden.len() {
        topo.sel += 1;
    }

    // Ajouter / retirer un neurone dans la couche sélectionnée.
    let sel = topo.sel;
    if keys.just_pressed(KeyCode::ArrowUp) {
        if let Some(n) = topo.hidden.get_mut(sel) {
            if *n < MAX_NODES {
                *n += 1;
                topo.dirty = true;
            }
        }
    }
    if keys.just_pressed(KeyCode::ArrowDown) {
        if let Some(n) = topo.hidden.get_mut(sel) {
            if *n > 1 {
                *n -= 1;
                topo.dirty = true;
            }
        }
    }

    // Ajouter / retirer une couche cachée.
    if keys.just_pressed(KeyCode::KeyN) && topo.hidden.len() < MAX_DEPTH {
        topo.hidden.push(5);
        topo.dirty = true;
    }
    if keys.just_pressed(KeyCode::KeyM) && topo.hidden.len() > 1 {
        topo.hidden.pop();
        if topo.sel >= topo.hidden.len() {
            topo.sel = topo.hidden.len() - 1;
        }
        topo.dirty = true;
    }

    // Changement de jeu de données.
    if keys.just_pressed(KeyCode::Digit1) {
        switch_dataset(&mut data, &mut topo, DatasetKind::Xor);
    }
    if keys.just_pressed(KeyCode::Digit2) {
        switch_dataset(&mut data, &mut topo, DatasetKind::Spirals);
    }
    if keys.just_pressed(KeyCode::Digit3) {
        switch_dataset(&mut data, &mut topo, DatasetKind::Circles);
    }
    if keys.just_pressed(KeyCode::Digit4) {
        switch_dataset(&mut data, &mut topo, DatasetKind::Regression);
    }

    // Vitesse d'animation (durée d'un segment).
    if keys.just_pressed(KeyCode::Equal) {
        anim.seg_dur = (anim.seg_dur * 0.8).clamp(0.05, 2.0);
    }
    if keys.just_pressed(KeyCode::Minus) {
        anim.seg_dur = (anim.seg_dur * 1.25).clamp(0.05, 2.0);
    }

    // Taux d'apprentissage.
    if keys.just_pressed(KeyCode::BracketRight) {
        hyper.lr = (hyper.lr * 1.3).clamp(0.001, 5.0);
    }
    if keys.just_pressed(KeyCode::BracketLeft) {
        hyper.lr = (hyper.lr / 1.3).clamp(0.001, 5.0);
    }
}
