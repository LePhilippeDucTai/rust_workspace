//! Overlay : cercles de vision dessinés via `Gizmos` quand `show_vision = true`.

use bevy::prelude::*;

use crate::components::{Genome, Organism};
use crate::resources::SimState;

pub struct OverlayPlugin;

impl Plugin for OverlayPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, draw_vision);
    }
}

fn draw_vision(
    state: Res<SimState>,
    mut gizmos: Gizmos,
    q: Query<(&Transform, &Genome), With<Organism>>,
) {
    if !state.show_vision {
        return;
    }
    let color = Color::srgba(1.0, 1.0, 1.0, 0.10);
    for (tf, genome) in &q {
        let p = tf.translation.truncate();
        gizmos.circle_2d(p, genome.vision_range, color);
    }
}
