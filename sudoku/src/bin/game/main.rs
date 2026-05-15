//! Sudoku en Bevy, style Studio Ghibli.
//! Lancement: `cargo run -p sudoku --bin game --release`

use bevy::prelude::*;

mod colors;
mod components;
mod logic;
mod systems;
mod ui;

use colors::{COL_BG, WIN_H, WIN_W};
use components::{EnterNumber, Redo, SelectCell, Selection, SudokuGame, Undo};
use systems::*;
use ui::setup;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Sudoku - un jardin tranquille".into(),
                resolution: (WIN_W, WIN_H).into(),
                resizable: false,
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(COL_BG))
        .insert_resource(SudokuGame::new())
        .insert_resource(Selection::default())
        .add_message::<SelectCell>()
        .add_message::<EnterNumber>()
        .add_message::<Undo>()
        .add_message::<Redo>()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                cell_click_system,
                number_pad_click_system,
                undo_click_system,
                redo_click_system,
                new_game_click_system,
                keyboard_input_system,
                handle_select_cell,
                handle_enter_number,
                handle_undo,
                handle_redo,
                update_cell_visuals,
                update_win_banner,
                button_visual_feedback,
            ),
        )
        .run();
}
