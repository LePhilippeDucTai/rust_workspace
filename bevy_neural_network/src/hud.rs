//! HUD : état de l'entraînement (en haut) et aide aux contrôles (en bas).

use bevy::prelude::*;

use crate::components::{HudText, StatsText};
use crate::training::{Anim, Data, Hyper, LossHist, Net, Steps, Topology};

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
        TextColor(Color::srgb(0.92, 0.92, 0.96)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(8.0),
            left: Val::Px(10.0),
            padding: UiRect::all(Val::Px(8.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.02, 0.02, 0.05, 0.55)),
        StatsText,
    ));

    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: 13.0,
            ..default()
        },
        TextColor(Color::srgb(0.7, 0.85, 0.95)),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(8.0),
            left: Val::Px(10.0),
            padding: UiRect::all(Val::Px(8.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.02, 0.02, 0.05, 0.55)),
        HudText,
    ));
}

fn update_hud(
    anim: Res<Anim>,
    net: Res<Net>,
    data: Res<Data>,
    hyper: Res<Hyper>,
    topo: Res<Topology>,
    steps: Res<Steps>,
    loss: Res<LossHist>,
    mut stats_q: Query<&mut Text, (With<StatsText>, Without<HudText>)>,
    mut hud_q: Query<&mut Text, (With<HudText>, Without<StatsText>)>,
) {
    let state = if anim.running { "LECTURE" } else { "PAUSE" };
    let current_loss = loss.0.last().copied().unwrap_or(0.0);

    // Topologie avec la couche cachée sélectionnée mise en évidence.
    let mut arch = format!("{}", net.0.sizes.first().copied().unwrap_or(0));
    for (k, &h) in topo.hidden.iter().enumerate() {
        if k == topo.sel {
            arch.push_str(&format!(" - [{h}]"));
        } else {
            arch.push_str(&format!(" - {h}"));
        }
    }
    arch.push_str(&format!(
        " - {}",
        net.0.sizes.last().copied().unwrap_or(0)
    ));

    if let Ok(mut t) = stats_q.single_mut() {
        **t = format!(
            "Visualisation d'un reseau de neurones en apprentissage\n\
             Dataset : {}        Etat : {}   Phase : {}\n\
             Architecture : {}\n\
             Pas SGD : {}     Loss : {:.5}     lr : {:.3}     vitesse : {:.2}s/couche",
            data.0.kind.label(),
            state,
            anim.phase.label(),
            arch,
            steps.0,
            current_loss,
            hyper.lr,
            anim.seg_dur,
        );
    }

    if let Ok(mut t) = hud_q.single_mut() {
        **t = "P lecture/pause   Espace pas-a-pas   F turbo (maintenir)   R reset\n\
               Gauche/Droite : couche selectionnee   Haut/Bas : +/- neurones   N/M : +/- couche\n\
               1 XOR   2 Spirales   3 Cercles   4 Regression     + / - vitesse     [ / ] taux d'apprentissage"
            .to_string();
    }
}
