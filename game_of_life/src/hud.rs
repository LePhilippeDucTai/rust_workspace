use bevy::prelude::*;

use crate::grid::Grid;
use crate::simulation::SimState;

#[derive(Component)]
struct HudText;

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_hud)
            .add_systems(Update, update_hud);
    }
}

fn spawn_hud(mut commands: Commands) {
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: 15.0,
            ..default()
        },
        TextColor(Color::srgba(1.0, 1.0, 1.0, 0.85)),
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
    sim: Res<SimState>,
    grid: Res<Grid>,
    fixed_time: Res<Time<Fixed>>,
    mut text_q: Query<&mut Text, With<HudText>>,
) {
    if !sim.is_changed() && !grid.is_changed() {
        return;
    }
    let Ok(mut text) = text_q.single_mut() else {
        return;
    };
    let hz = fixed_time.timestep().as_secs_f64().recip();
    let status = if sim.paused { "PAUSED " } else { "RUNNING" };
    **text = format!(
        "[Space] {status}  |  Gen: {}  |  Alive: {}  |  Speed: {hz:.0} Hz\n\
         [R] Randomize  [C] Clear  [↑↓] Speed  [LClick] Draw",
        sim.generation,
        grid.alive_count(),
    );
}
