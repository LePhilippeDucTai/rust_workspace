//! Affichage du HUD : état de la simulation et légende des contrôles.

use bevy::prelude::*;

use crate::components::{Ball, HudText};
use crate::resources::SimSettings;

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_hud)
            .add_systems(Update, update_hud);
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
    settings: Res<SimSettings>,
    balls: Query<&Ball>,
    mut text_q: Query<&mut Text, With<HudText>>,
    time: Res<Time>,
) {
    let Ok(mut text) = text_q.single_mut() else {
        return;
    };
    let count = balls.iter().count();
    let fps = 1.0 / time.delta_secs().max(1e-6);
    let on = |b: bool| if b { "ON" } else { "OFF" };
    **text = format!(
        "Balls: {count}   FPS: {fps:.0}\n\
         Gravity [G]: {}   Collisions [A]: {}   Paused [Space]: {}\n\
         Clic gauche: ajouter une balle   [R]: reset   [T]: trace balle",
        on(settings.gravity_enabled),
        on(settings.collisions_enabled),
        on(settings.paused),
    );
}
