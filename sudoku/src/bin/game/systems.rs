use bevy::prelude::*;

use crate::colors::*;
use crate::components::*;
use crate::logic::{has_conflict, is_complete_and_valid};

pub fn cell_click_system(
    mut q: Query<(&Interaction, &Cell), Changed<Interaction>>,
    mut writer: MessageWriter<SelectCell>,
) {
    for (i, cell) in &mut q {
        if *i == Interaction::Pressed {
            writer.write(SelectCell { row: cell.row, col: cell.col });
        }
    }
}

pub fn number_pad_click_system(
    mut q: Query<(&Interaction, &NumberPad), Changed<Interaction>>,
    mut writer: MessageWriter<EnterNumber>,
) {
    for (i, np) in &mut q {
        if *i == Interaction::Pressed {
            writer.write(EnterNumber(np.0));
        }
    }
}

pub fn undo_click_system(
    mut q: Query<&Interaction, (Changed<Interaction>, With<UndoButton>)>,
    mut writer: MessageWriter<Undo>,
) {
    for i in &mut q {
        if *i == Interaction::Pressed {
            writer.write(Undo);
        }
    }
}

pub fn redo_click_system(
    mut q: Query<&Interaction, (Changed<Interaction>, With<RedoButton>)>,
    mut writer: MessageWriter<Redo>,
) {
    for i in &mut q {
        if *i == Interaction::Pressed {
            writer.write(Redo);
        }
    }
}

pub fn new_game_click_system(
    mut q: Query<&Interaction, (Changed<Interaction>, With<NewGameButton>)>,
    mut game: ResMut<SudokuGame>,
    mut selection: ResMut<Selection>,
) {
    for i in &mut q {
        if *i == Interaction::Pressed {
            *game = SudokuGame::new();
            selection.0 = None;
        }
    }
}

pub fn keyboard_input_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut enter_writer: MessageWriter<EnterNumber>,
    mut undo_writer: MessageWriter<Undo>,
    mut redo_writer: MessageWriter<Redo>,
    mut selection: ResMut<Selection>,
) {
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight)
        || keys.pressed(KeyCode::SuperLeft) || keys.pressed(KeyCode::SuperRight);

    if ctrl && keys.just_pressed(KeyCode::KeyZ) {
        if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
            redo_writer.write(Redo);
        } else {
            undo_writer.write(Undo);
        }
        return;
    }
    if ctrl && keys.just_pressed(KeyCode::KeyY) {
        redo_writer.write(Redo);
        return;
    }

    let digit_keys = [
        (KeyCode::Digit1, 1u8), (KeyCode::Digit2, 2), (KeyCode::Digit3, 3),
        (KeyCode::Digit4, 4),   (KeyCode::Digit5, 5), (KeyCode::Digit6, 6),
        (KeyCode::Digit7, 7),   (KeyCode::Digit8, 8), (KeyCode::Digit9, 9),
        (KeyCode::Numpad1, 1),  (KeyCode::Numpad2, 2), (KeyCode::Numpad3, 3),
        (KeyCode::Numpad4, 4),  (KeyCode::Numpad5, 5), (KeyCode::Numpad6, 6),
        (KeyCode::Numpad7, 7),  (KeyCode::Numpad8, 8), (KeyCode::Numpad9, 9),
    ];
    for (kc, v) in digit_keys {
        if keys.just_pressed(kc) {
            enter_writer.write(EnterNumber(v));
        }
    }
    if keys.just_pressed(KeyCode::Backspace)
        || keys.just_pressed(KeyCode::Delete)
        || keys.just_pressed(KeyCode::Digit0)
        || keys.just_pressed(KeyCode::Numpad0)
        || keys.just_pressed(KeyCode::Space)
    {
        enter_writer.write(EnterNumber(0));
    }

    if let Some((r, c)) = selection.0 {
        let mut nr = r as i32;
        let mut nc = c as i32;
        if keys.just_pressed(KeyCode::ArrowUp)    { nr -= 1; }
        if keys.just_pressed(KeyCode::ArrowDown)  { nr += 1; }
        if keys.just_pressed(KeyCode::ArrowLeft)  { nc -= 1; }
        if keys.just_pressed(KeyCode::ArrowRight) { nc += 1; }
        let nrc = (nr.clamp(0, 8) as u8, nc.clamp(0, 8) as u8);
        if nrc != (r, c) {
            selection.0 = Some(nrc);
        }
    } else if keys.just_pressed(KeyCode::ArrowUp)
        || keys.just_pressed(KeyCode::ArrowDown)
        || keys.just_pressed(KeyCode::ArrowLeft)
        || keys.just_pressed(KeyCode::ArrowRight)
    {
        selection.0 = Some((4, 4));
    }
}

pub fn handle_select_cell(
    mut reader: MessageReader<SelectCell>,
    mut selection: ResMut<Selection>,
) {
    for ev in reader.read() {
        selection.0 = Some((ev.row, ev.col));
    }
}

pub fn handle_enter_number(
    mut reader: MessageReader<EnterNumber>,
    selection: Res<Selection>,
    mut game: ResMut<SudokuGame>,
) {
    for ev in reader.read() {
        if game.won { continue; }
        let Some((r, c)) = selection.0 else { continue };
        let (r, c) = (r as usize, c as usize);
        if game.given[r][c] { continue; }
        let prev = game.current[r][c];
        if prev == ev.0 { continue; }
        game.history.push((r as u8, c as u8, prev));
        game.future.clear();
        game.current[r][c] = ev.0;
        game.won = is_complete_and_valid(&game.current);
    }
}

pub fn handle_undo(
    mut reader: MessageReader<Undo>,
    mut game: ResMut<SudokuGame>,
    mut selection: ResMut<Selection>,
) {
    for _ in reader.read() {
        let Some((r, c, prev)) = game.history.pop() else { continue };
        let current_val = game.current[r as usize][c as usize];
        game.future.push((r, c, current_val));
        game.current[r as usize][c as usize] = prev;
        game.won = is_complete_and_valid(&game.current);
        selection.0 = Some((r, c));
    }
}

pub fn handle_redo(
    mut reader: MessageReader<Redo>,
    mut game: ResMut<SudokuGame>,
    mut selection: ResMut<Selection>,
) {
    for _ in reader.read() {
        let Some((r, c, next)) = game.future.pop() else { continue };
        let current_val = game.current[r as usize][c as usize];
        game.history.push((r, c, current_val));
        game.current[r as usize][c as usize] = next;
        game.won = is_complete_and_valid(&game.current);
        selection.0 = Some((r, c));
    }
}

pub fn update_cell_visuals(
    game: Res<SudokuGame>,
    selection: Res<Selection>,
    mut cells: Query<(&Cell, &mut BackgroundColor, &Children)>,
    mut texts: Query<(&mut Text, &mut TextColor), With<CellText>>,
) {
    let sel = selection.0;
    let sel_num = sel
        .map(|(r, c)| game.current[r as usize][c as usize])
        .unwrap_or(0);

    for (cell, mut bg, children) in &mut cells {
        let r = cell.row as usize;
        let c = cell.col as usize;
        let value = game.current[r][c];
        let given = game.given[r][c];

        let mut color = if given { COL_CELL_GIVEN } else { COL_CELL };
        if let Some((sr, sc)) = sel {
            let sr_u = sr as usize;
            let sc_u = sc as usize;
            let same_block = (r / 3 == sr_u / 3) && (c / 3 == sc_u / 3);
            if r == sr_u && c == sc_u {
                color = COL_SELECTED;
            } else if value != 0 && sel_num != 0 && value == sel_num {
                color = COL_SAME_NUM;
            } else if r == sr_u || c == sc_u || same_block {
                color = COL_HIGHLIGHT;
            }
        }
        bg.0 = color;

        let conflict = has_conflict(&game.current, r, c);
        for child in children.iter() {
            if let Ok((mut text, mut tcolor)) = texts.get_mut(child) {
                **text = if value == 0 { String::new() } else { value.to_string() };
                tcolor.0 = if conflict {
                    COL_TEXT_CONFLICT
                } else if given {
                    COL_TEXT_GIVEN
                } else {
                    COL_TEXT_USER
                };
            }
        }
    }
}

pub fn update_win_banner(game: Res<SudokuGame>, mut q: Query<&mut Node, With<WinBanner>>) {
    if !game.is_changed() { return; }
    for mut node in &mut q {
        node.display = if game.won { Display::Flex } else { Display::None };
    }
}

pub fn button_visual_feedback(
    mut q: Query<
        (&Interaction, &ButtonStyle, &mut BackgroundColor),
        (Changed<Interaction>, Without<Cell>),
    >,
) {
    for (i, style, mut bg) in &mut q {
        bg.0 = match *i {
            Interaction::Pressed => style.pressed,
            Interaction::Hovered => style.hover,
            Interaction::None => style.default,
        };
    }
}
