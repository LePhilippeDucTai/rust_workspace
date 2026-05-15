use bevy::prelude::*;

use crate::colors::N;

#[derive(Resource)]
pub struct SudokuGame {
    pub current: [[u8; N]; N],
    pub given: [[bool; N]; N],
    pub won: bool,
    /// (row, col, valeur avant le coup) — pour annuler
    pub history: Vec<(u8, u8, u8)>,
    /// (row, col, valeur après le coup) — pour refaire
    pub future: Vec<(u8, u8, u8)>,
}

#[derive(Resource, Default)]
pub struct Selection(pub Option<(u8, u8)>);

#[derive(Component, Copy, Clone)]
pub struct Cell {
    pub row: u8,
    pub col: u8,
}

#[derive(Component)]
pub struct CellText;

#[derive(Component, Copy, Clone)]
pub struct NumberPad(pub u8);

#[derive(Component)]
pub struct NewGameButton;

#[derive(Component)]
pub struct UndoButton;

#[derive(Component)]
pub struct RedoButton;

#[derive(Component)]
pub struct WinBanner;

#[derive(Component, Copy, Clone)]
pub struct ButtonStyle {
    pub default: Color,
    pub hover: Color,
    pub pressed: Color,
}

#[derive(Message)]
pub struct SelectCell {
    pub row: u8,
    pub col: u8,
}

#[derive(Message)]
pub struct EnterNumber(pub u8);

#[derive(Message)]
pub struct Undo;

#[derive(Message)]
pub struct Redo;
