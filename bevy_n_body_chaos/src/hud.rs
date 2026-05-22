use bevy::prelude::*;

use crate::components::HudText;
use crate::resources::{Energy, SimSettings};

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
        TextFont { font_size: 14.0, ..default() },
        TextColor(Color::srgb(0.9, 0.92, 1.0)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            left: Val::Px(14.0),
            ..default()
        },
        HudText,
    ));
}

fn update_hud(
    settings: Res<SimSettings>,
    energy: Res<Energy>,
    time: Res<Time>,
    mut q: Query<&mut Text, With<HudText>>,
) {
    let Ok(mut text) = q.single_mut() else { return; };
    let fps = 1.0 / time.delta_secs().max(1e-6);
    let scenario_names = ["Figure en 8", "Triangle Pythagorique", "Triangle de Lagrange", "Aléatoire 3D"];
    let name = scenario_names.get(settings.scenario_idx).copied().unwrap_or("?");
    let state = if settings.paused { "PAUSE" } else { "  RUN" };
    let total_e = energy.ke + energy.pe;
    **text = format!(
        "N-Body Chaos  |  {state}  |  FPS {fps:4.0}\n\
         Scénario [{idx}] : {name}\n\
         G = {g:.0}   Softening ε = 1.5\n\
         Énergie cinétique  : {ke:+.2}\n\
         Énergie potentielle: {pe:+.2}\n\
         Énergie totale     : {total_e:+.2}\n\
         \n\
         [1-4] Changer scénario   [R] Reset\n\
         [Espace] Pause   [Souris] Orbiter   [Scroll] Zoom",
        idx = settings.scenario_idx + 1,
        g = settings.g,
        ke = energy.ke,
        pe = energy.pe,
    );
}
