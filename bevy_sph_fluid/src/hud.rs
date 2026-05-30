//! HUD : état de la simulation, contour du conteneur, légende des contrôles.

use bevy::prelude::*;

use crate::components::HudText;
use crate::config::{BOX_BOTTOM, BOX_LEFT, BOX_RIGHT, BOX_TOP};
use crate::resources::{FluidSim, SimSettings};

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_hud)
            .add_systems(Update, (update_hud, draw_container));
    }
}

fn setup_hud(mut commands: Commands) {
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: 14.0,
            ..default()
        },
        TextColor(Color::srgb(0.9, 0.9, 0.95)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(8.0),
            left: Val::Px(10.0),
            ..default()
        },
        HudText,
    ));
}

fn update_hud(
    sim: Res<FluidSim>,
    settings: Res<SimSettings>,
    time: Res<Time>,
    mut text_q: Query<&mut Text, With<HudText>>,
) {
    let Ok(mut text) = text_q.single_mut() else {
        return;
    };
    let fps = 1.0 / time.delta_secs().max(1e-6);
    let on = |b: bool| if b { "ON" } else { "OFF" };
    // HUD en ASCII : la police par défaut de Bevy ne couvre pas les
    // diacritiques ni les symboles grecs/indices.
    **text = format!(
        "Particules: {}   FPS: {fps:.0}   rho0: {:.3}\n\
         Gravite [G]: {}   Viscosite [V]: {}   Pause [Espace]: {}\n\
         Clic gauche: repousser   Clic droit: attirer   [R]: reinitialiser",
        sim.0.len(),
        sim.0.rest_density,
        on(settings.gravity_enabled),
        on(settings.viscosity_enabled),
        on(settings.paused),
    );
}

/// Dessine le contour du conteneur du fluide.
fn draw_container(mut gizmos: Gizmos) {
    let center = Vec2::new((BOX_LEFT + BOX_RIGHT) * 0.5, (BOX_BOTTOM + BOX_TOP) * 0.5);
    let size = Vec2::new(BOX_RIGHT - BOX_LEFT, BOX_TOP - BOX_BOTTOM);
    gizmos.rect_2d(
        Isometry2d::from_translation(center),
        size,
        Color::srgb(0.35, 0.35, 0.45),
    );
}
