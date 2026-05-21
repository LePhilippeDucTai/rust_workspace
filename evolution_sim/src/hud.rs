//! HUD : panneau de stats en haut-gauche + aide en bas-gauche.
//!
//! On utilise deux entités texte distinctes pour éviter de reformatter une
//! grosse chaîne quand seule la stat change.

use bevy::prelude::*;

use crate::components::{HudText, StatsText};
use crate::resources::{PopulationStats, SimState};

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_hud)
            .add_systems(Update, (update_stats_text, update_help_text));
    }
}

fn setup_hud(mut commands: Commands) {
    // Bloc stats (haut-gauche).
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: 14.0,
            ..default()
        },
        TextColor(Color::srgb(0.92, 0.95, 1.0)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(8.0),
            left: Val::Px(10.0),
            ..default()
        },
        StatsText,
    ));

    // Bloc aide / contrôles (bas-gauche).
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: 12.0,
            ..default()
        },
        TextColor(Color::srgb(0.70, 0.74, 0.82)),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(8.0),
            left: Val::Px(10.0),
            ..default()
        },
        HudText,
    ));
}

fn update_stats_text(
    stats: Res<PopulationStats>,
    state: Res<SimState>,
    time: Res<Time>,
    mut q: Query<&mut Text, With<StatsText>>,
) {
    let Ok(mut text) = q.single_mut() else {
        return;
    };
    let fps = 1.0 / time.delta_secs().max(1e-6);
    **text = format!(
        "FPS: {fps:>3.0}    Sim: {t:>6.1}s    Pop: {pop:>3}    Food: {food:>3}    Births: {b}    Deaths: {d}\n\
         Generation  max: {gmax:>3}    moyenne: {gmean:>5.1}\n\
         Vitesse  moy: {vs:>6.1}    Taille moy: {sz:>5.2}    Vision moy: {vis:>6.1}\n\
         Énergie  moy: {en:>6.1}    {pause}",
        t = state.elapsed,
        pop = stats.count,
        food = stats.food_count,
        b = state.total_births,
        d = state.total_deaths,
        gmax = stats.max_generation,
        gmean = stats.mean_generation,
        vs = stats.mean_speed,
        sz = stats.mean_size,
        vis = stats.mean_vision,
        en = stats.mean_energy,
        pause = if state.paused { "[PAUSE]" } else { "" },
    );
}

fn update_help_text(state: Res<SimState>, mut q: Query<&mut Text, With<HudText>>) {
    let Ok(mut text) = q.single_mut() else {
        return;
    };
    if !state.show_help {
        **text = String::new();
        return;
    }
    **text = String::from(
        "[Espace] pause   [V] afficher portée de vision   [H] cacher cette aide\n\
         Teinte = vitesse (rouge=rapide, bleu=lent)   Saturation = vision   Taille = gène taille",
    );
}
