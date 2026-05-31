//! Visualisation interactive d'un réseau de neurones en cours d'apprentissage.
//!
//! L'application matérialise la descente de gradient pas à pas : on voit l'onde
//! de la propagation avant traverser les couches, l'onde des gradients remonter
//! pendant la rétropropagation, puis les poids (couleur et opacité des arêtes)
//! se mettre à jour. On peut modifier en direct le nombre de neurones et la
//! profondeur du réseau, changer de jeu de données, et observer la frontière de
//! décision (ou la courbe de régression) se former en même temps que la loss
//! décroît.
//!
//! Tout est écrit avec Bevy. Voir le HUD en bas de fenêtre pour les contrôles.

mod components;
mod config;
mod dataset;
mod hud;
mod input;
mod layout;
mod network;
mod training;
mod visualization;

use bevy::prelude::*;

use crate::config::{WINDOW_HEIGHT, WINDOW_WIDTH};
use crate::dataset::{Dataset, DatasetKind};
use crate::hud::HudPlugin;
use crate::input::InputPlugin;
use crate::network::{Network, OutAct};
use crate::training::{
    Anim, Data, GraphDirty, Hyper, LossHist, Net, Steps, Topology, Turbo, advance_training,
    rebuild_network,
};
use crate::visualization::VisualizationPlugin;

fn main() {
    // État initial cohérent : dataset XOR, deux couches cachées de 5 neurones.
    let data = Dataset::new(DatasetKind::Xor);
    let topo = Topology::default();
    let mut sizes = vec![data.in_dim];
    sizes.extend(topo.hidden.iter().copied());
    sizes.push(data.out_dim);
    let net = Network::new(&sizes, OutAct::Sigmoid);

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Bevy - Reseau de neurones en apprentissage".into(),
                resolution: (WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32).into(),
                resizable: false,
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::srgb(0.05, 0.05, 0.08)))
        .insert_resource(Net(net))
        .insert_resource(Data(data))
        .insert_resource(topo)
        .insert_resource(Anim::default())
        .insert_resource(Hyper::default())
        .insert_resource(LossHist::default())
        .insert_resource(Steps::default())
        .insert_resource(Turbo::default())
        // Construit les entités neurones dès la première image.
        .insert_resource(GraphDirty(true))
        .add_plugins((VisualizationPlugin, HudPlugin, InputPlugin))
        .add_systems(Startup, setup_camera)
        .add_systems(Update, (rebuild_network, advance_training).chain())
        .run();
}

fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}
