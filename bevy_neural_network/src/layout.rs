//! Mise en page du graphe : positions des neurones et rayon.
//!
//! Source unique de vérité : utilisée à la fois pour placer les entités
//! neurones et pour tracer les arêtes/impulsions en mode immédiat (gizmos).

use bevy::prelude::*;

use crate::config::{NETWORK_RECT, lerp};

const PAD: f32 = 45.0;

/// Position monde du neurone `idx` de la couche `layer`.
pub fn neuron_pos(layer: usize, idx: usize, sizes: &[usize]) -> Vec2 {
    let rect = NETWORK_RECT;
    let lcount = sizes.len();
    let x = if lcount <= 1 {
        rect.center().x
    } else {
        lerp(
            rect.min.x + PAD,
            rect.max.x - PAD,
            layer as f32 / (lcount - 1) as f32,
        )
    };

    let n = sizes[layer];
    let cy = rect.center().y;
    // Espacement borné : ni trop serré pour beaucoup de neurones, ni trop
    // étalé pour quelques-uns.
    let max_n = sizes.iter().copied().max().unwrap_or(1).max(1) as f32;
    let avail = (rect.max.y - rect.min.y) - 2.0 * PAD;
    let spacing = (avail / max_n).min(64.0);
    let y = cy + (idx as f32 - (n as f32 - 1.0) / 2.0) * spacing;
    Vec2::new(x, y)
}

/// Rayon des neurones, réduit quand une couche est très peuplée.
pub fn neuron_radius(sizes: &[usize]) -> f32 {
    let max_n = sizes.iter().copied().max().unwrap_or(1).max(1) as f32;
    (260.0 / max_n).clamp(7.0, 22.0)
}
