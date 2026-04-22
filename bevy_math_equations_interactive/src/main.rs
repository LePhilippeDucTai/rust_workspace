use bevy::prelude::*;

mod ast;
mod parser;
mod transform;
mod layout;
mod animation;
mod render;
mod interaction;

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Équations Interactives".to_string(),
                        resolution: (1400.0, 820.0).into(),
                        ..default()
                    }),
                    ..default()
                })
                .set(ImagePlugin::default_nearest()),
        )
        .insert_resource(ClearColor(Color::srgb(0.03, 0.03, 0.05)))
        .add_plugins((
            animation::AnimationPlugin,
            render::RenderPlugin,
            interaction::InteractionPlugin,
        ))
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
}
