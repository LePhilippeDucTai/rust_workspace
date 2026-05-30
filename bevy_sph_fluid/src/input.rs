//! Entrées utilisateur : bascules clavier, réinitialisation, forçage souris.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::resources::{FluidSim, InitialLayout, Interaction, SimSettings};

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Interaction>().add_systems(
            Update,
            (handle_toggles, handle_reset, handle_mouse),
        );
    }
}

fn handle_toggles(keys: Res<ButtonInput<KeyCode>>, mut settings: ResMut<SimSettings>) {
    if keys.just_pressed(KeyCode::KeyG) {
        settings.gravity_enabled = !settings.gravity_enabled;
    }
    if keys.just_pressed(KeyCode::KeyV) {
        settings.viscosity_enabled = !settings.viscosity_enabled;
    }
    if keys.just_pressed(KeyCode::Space) {
        settings.paused = !settings.paused;
    }
}

/// `R` : restaure la disposition initiale et annule les vitesses.
fn handle_reset(
    keys: Res<ButtonInput<KeyCode>>,
    mut sim: ResMut<FluidSim>,
    initial: Res<InitialLayout>,
) {
    if !keys.just_pressed(KeyCode::KeyR) {
        return;
    }
    let fluid = &mut sim.0;
    fluid.pos.copy_from_slice(&initial.0);
    for v in fluid.vel.iter_mut() {
        *v = Vec2::ZERO;
    }
}

/// Clic gauche : repousse le fluide autour du curseur ; clic droit : l'attire.
fn handle_mouse(
    buttons: Res<ButtonInput<MouseButton>>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform)>,
    mut interaction: ResMut<Interaction>,
) {
    let left = buttons.pressed(MouseButton::Left);
    let right = buttons.pressed(MouseButton::Right);
    if !left && !right {
        interaction.active = false;
        return;
    }

    let Ok(window) = window_q.single() else {
        interaction.active = false;
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        interaction.active = false;
        return;
    };
    let Ok((camera, cam_tf)) = camera_q.single() else {
        interaction.active = false;
        return;
    };
    let Ok(world) = camera.viewport_to_world_2d(cam_tf, cursor) else {
        interaction.active = false;
        return;
    };

    interaction.active = true;
    interaction.center = world;
    interaction.repel = left;
}
