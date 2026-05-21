//! Entrées clavier : pause, affichage des cercles de vision, aide.

use bevy::prelude::*;

use crate::resources::SimState;

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, handle_keys);
    }
}

fn handle_keys(keys: Res<ButtonInput<KeyCode>>, mut state: ResMut<SimState>) {
    if keys.just_pressed(KeyCode::Space) {
        state.paused = !state.paused;
    }
    if keys.just_pressed(KeyCode::KeyV) {
        state.show_vision = !state.show_vision;
    }
    if keys.just_pressed(KeyCode::KeyH) {
        state.show_help = !state.show_help;
    }
}
