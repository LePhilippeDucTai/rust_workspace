//! Simulation 2D de balles rebondissantes avec Bevy.

mod components;
mod config;
mod hud;
mod input;
mod physics;
mod resources;
mod spawn;
mod trace;

use bevy::prelude::*;

use crate::config::{PHYSICS_HZ, WINDOW_HEIGHT, WINDOW_WIDTH};
use crate::hud::HudPlugin;
use crate::input::InputPlugin;
use crate::physics::PhysicsPlugin;
use crate::resources::SimSettings;
use crate::spawn::SpawnPlugin;
use crate::trace::TracePlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Bevy Bouncing Balls".into(),
                resolution: (WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32).into(),
                resizable: false,
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::srgb(0.08, 0.08, 0.12)))
        .insert_resource(SimSettings::default())
        .insert_resource(Time::<Fixed>::from_hz(PHYSICS_HZ))
        .add_plugins((SpawnPlugin, PhysicsPlugin, InputPlugin, HudPlugin, TracePlugin))
        .run();
}
