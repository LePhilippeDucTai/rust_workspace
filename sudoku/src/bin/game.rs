//! Sudoku en Bevy, style Studio Ghibli.
//! Lancement: `cargo run -p sudoku --bin game --release`

use bevy::prelude::*;
use rand::rng;
use rand::seq::SliceRandom;

const N: usize = 9;
const CELL_SIZE: f32 = 58.0;
const WIN_W: u32 = 900;
const WIN_H: u32 = 820;

// Palette inspirée Ghibli — crèmes chauds, sauge, terracotta.
const COL_BG: Color = Color::srgb(0.96, 0.91, 0.80);
const COL_GRID: Color = Color::srgb(0.39, 0.28, 0.20);
const COL_BLOCK_BG: Color = Color::srgb(0.55, 0.41, 0.30);
const COL_CELL: Color = Color::srgb(0.995, 0.965, 0.88);
const COL_CELL_GIVEN: Color = Color::srgb(0.93, 0.88, 0.74);
const COL_SELECTED: Color = Color::srgb(0.74, 0.84, 0.62);
const COL_HIGHLIGHT: Color = Color::srgb(0.87, 0.92, 0.76);
const COL_SAME_NUM: Color = Color::srgb(0.80, 0.88, 0.66);
const COL_TEXT_GIVEN: Color = Color::srgb(0.17, 0.30, 0.21);
const COL_TEXT_USER: Color = Color::srgb(0.18, 0.36, 0.55);
const COL_TEXT_CONFLICT: Color = Color::srgb(0.74, 0.27, 0.19);
const COL_TERRACOTTA: Color = Color::srgb(0.80, 0.45, 0.30);
const COL_LEAF: Color = Color::srgb(0.45, 0.62, 0.40);
const COL_LEAF_DARK: Color = Color::srgb(0.32, 0.48, 0.30);
const COL_TITLE: Color = Color::srgb(0.45, 0.27, 0.17);
const COL_SUBTITLE: Color = Color::srgb(0.55, 0.40, 0.28);
const COL_WHITE: Color = Color::srgb(1.0, 0.98, 0.94);

#[derive(Resource)]
struct SudokuGame {
    current: [[u8; N]; N],
    given: [[bool; N]; N],
    won: bool,
}

#[derive(Resource, Default)]
struct Selection(Option<(u8, u8)>);

#[derive(Component, Copy, Clone)]
struct Cell {
    row: u8,
    col: u8,
}

#[derive(Component)]
struct CellText;

#[derive(Component, Copy, Clone)]
struct NumberPad(u8); // 0 = clear

#[derive(Component)]
struct NewGameButton;

#[derive(Component)]
struct WinBanner;

#[derive(Component, Copy, Clone)]
struct ButtonStyle {
    default: Color,
    hover: Color,
    pressed: Color,
}

#[derive(Message)]
struct SelectCell {
    row: u8,
    col: u8,
}

#[derive(Message)]
struct EnterNumber(u8);

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Sudoku — un jardin tranquille".into(),
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
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                cell_click_system,
                number_pad_click_system,
                new_game_click_system,
                keyboard_input_system,
                handle_select_cell,
                handle_enter_number,
                update_cell_visuals,
                update_win_banner,
                button_visual_feedback,
            ),
        )
        .run();
}

// ---------- Logique du puzzle ----------

impl SudokuGame {
    fn new() -> Self {
        let solution = generate_solution();
        let mask = make_puzzle_mask(35);
        let mut current = [[0u8; N]; N];
        let mut given = [[false; N]; N];
        for r in 0..N {
            for c in 0..N {
                if mask[r][c] {
                    current[r][c] = solution[r][c];
                    given[r][c] = true;
                }
            }
        }
        Self {
            current,
            given,
            won: false,
        }
    }
}

fn generate_solution() -> [[u8; N]; N] {
    let base: [[u8; N]; N] = [
        [1, 2, 3, 4, 5, 6, 7, 8, 9],
        [4, 5, 6, 7, 8, 9, 1, 2, 3],
        [7, 8, 9, 1, 2, 3, 4, 5, 6],
        [2, 3, 1, 5, 6, 4, 8, 9, 7],
        [5, 6, 4, 8, 9, 7, 2, 3, 1],
        [8, 9, 7, 2, 3, 1, 5, 6, 4],
        [3, 1, 2, 6, 4, 5, 9, 7, 8],
        [6, 4, 5, 9, 7, 8, 3, 1, 2],
        [9, 7, 8, 3, 1, 2, 6, 4, 5],
    ];
    let mut r = rng();

    // Permutation des chiffres 1..=9
    let mut digits: [u8; 9] = [1, 2, 3, 4, 5, 6, 7, 8, 9];
    digits.shuffle(&mut r);
    let mut g = [[0u8; N]; N];
    for i in 0..N {
        for j in 0..N {
            g[i][j] = digits[(base[i][j] - 1) as usize];
        }
    }

    // Permutation des rangées à l'intérieur de chaque bande horizontale
    for band in 0..3 {
        let mut p = [0usize, 1, 2];
        p.shuffle(&mut r);
        let original = g;
        for i in 0..3 {
            g[band * 3 + i] = original[band * 3 + p[i]];
        }
    }
    // Permutation des colonnes à l'intérieur de chaque bande verticale
    for band in 0..3 {
        let mut p = [0usize, 1, 2];
        p.shuffle(&mut r);
        let original = g;
        for i in 0..N {
            for j in 0..3 {
                g[i][band * 3 + j] = original[i][band * 3 + p[j]];
            }
        }
    }
    // Permutation des bandes horizontales
    {
        let mut p = [0usize, 1, 2];
        p.shuffle(&mut r);
        let original = g;
        for b in 0..3 {
            for i in 0..3 {
                g[b * 3 + i] = original[p[b] * 3 + i];
            }
        }
    }
    // Permutation des bandes verticales
    {
        let mut p = [0usize, 1, 2];
        p.shuffle(&mut r);
        let original = g;
        for b in 0..3 {
            for j in 0..3 {
                for i in 0..N {
                    g[i][b * 3 + j] = original[i][p[b] * 3 + j];
                }
            }
        }
    }
    g
}

fn make_puzzle_mask(clues: usize) -> [[bool; N]; N] {
    let mut positions: Vec<(usize, usize)> = (0..81).map(|i| (i / 9, i % 9)).collect();
    positions.shuffle(&mut rng());
    let mut mask = [[false; N]; N];
    for &(r, c) in positions.iter().take(clues) {
        mask[r][c] = true;
    }
    mask
}

fn has_conflict(b: &[[u8; N]; N], r: usize, c: usize) -> bool {
    let v = b[r][c];
    if v == 0 {
        return false;
    }
    for i in 0..N {
        if i != c && b[r][i] == v {
            return true;
        }
        if i != r && b[i][c] == v {
            return true;
        }
    }
    let br = (r / 3) * 3;
    let bc = (c / 3) * 3;
    for i in br..br + 3 {
        for j in bc..bc + 3 {
            if (i, j) != (r, c) && b[i][j] == v {
                return true;
            }
        }
    }
    false
}

fn is_complete_and_valid(b: &[[u8; N]; N]) -> bool {
    for r in 0..N {
        for c in 0..N {
            if b[r][c] == 0 || has_conflict(b, r, c) {
                return false;
            }
        }
    }
    true
}

// ---------- Setup de la scène ----------

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);

    // Quelques feuillages décoratifs en fond, pour la touche Ghibli.
    spawn_leaf(&mut commands, 60.0, 720.0, 120.0, 70.0, COL_LEAF, -0.3);
    spawn_leaf(&mut commands, 820.0, 740.0, 130.0, 80.0, COL_LEAF_DARK, 0.4);
    spawn_leaf(&mut commands, 30.0, 80.0, 90.0, 50.0, COL_LEAF_DARK, 0.5);
    spawn_leaf(&mut commands, 830.0, 70.0, 90.0, 55.0, COL_LEAF, -0.4);

    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            top: Val::Px(0.0),
            left: Val::Px(0.0),
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(10.0),
            padding: UiRect::all(Val::Px(18.0)),
            ..default()
        })
        .with_children(|root| {
            // Titre
            root.spawn((
                Text::new("Sudoku au village"),
                TextFont {
                    font_size: 38.0,
                    ..default()
                },
                TextColor(COL_TITLE),
            ));
            root.spawn((
                Text::new("~ une promenade chiffrée ~"),
                TextFont {
                    font_size: 15.0,
                    ..default()
                },
                TextColor(COL_SUBTITLE),
            ));

            // Plateau: 3x3 blocs de 3x3 cases
            root.spawn((
                Node {
                    display: Display::Grid,
                    grid_template_columns: vec![GridTrack::auto(); 3],
                    grid_template_rows: vec![GridTrack::auto(); 3],
                    row_gap: Val::Px(4.0),
                    column_gap: Val::Px(4.0),
                    padding: UiRect::all(Val::Px(8.0)),
                    border_radius: BorderRadius::all(Val::Px(14.0)),
                    margin: UiRect::top(Val::Px(6.0)),
                    ..default()
                },
                BackgroundColor(COL_GRID),
            ))
            .with_children(|board| {
                for br in 0..3u8 {
                    for bc in 0..3u8 {
                        board
                            .spawn((
                                Node {
                                    display: Display::Grid,
                                    grid_template_columns: vec![GridTrack::auto(); 3],
                                    grid_template_rows: vec![GridTrack::auto(); 3],
                                    row_gap: Val::Px(2.0),
                                    column_gap: Val::Px(2.0),
                                    border_radius: BorderRadius::all(Val::Px(6.0)),
                                    padding: UiRect::all(Val::Px(2.0)),
                                    ..default()
                                },
                                BackgroundColor(COL_BLOCK_BG),
                            ))
                            .with_children(|block| {
                                for r in 0..3u8 {
                                    for c in 0..3u8 {
                                        let row = br * 3 + r;
                                        let col = bc * 3 + c;
                                        block
                                            .spawn((
                                                Button,
                                                Node {
                                                    width: Val::Px(CELL_SIZE),
                                                    height: Val::Px(CELL_SIZE),
                                                    justify_content: JustifyContent::Center,
                                                    align_items: AlignItems::Center,
                                                    border_radius: BorderRadius::all(Val::Px(4.0)),
                                                    ..default()
                                                },
                                                BackgroundColor(COL_CELL),
                                                Cell { row, col },
                                            ))
                                            .with_children(|c2| {
                                                c2.spawn((
                                                    Text::new(""),
                                                    TextFont {
                                                        font_size: 30.0,
                                                        ..default()
                                                    },
                                                    TextColor(COL_TEXT_USER),
                                                    CellText,
                                                ));
                                            });
                                    }
                                }
                            });
                    }
                }
            });

            // Pavé numérique
            root.spawn(Node {
                display: Display::Flex,
                column_gap: Val::Px(6.0),
                margin: UiRect::top(Val::Px(10.0)),
                ..default()
            })
            .with_children(|pad| {
                for n in 1..=9u8 {
                    pad.spawn((
                        Button,
                        Node {
                            width: Val::Px(50.0),
                            height: Val::Px(50.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            border_radius: BorderRadius::all(Val::Px(10.0)),
                            ..default()
                        },
                        BackgroundColor(COL_CELL_GIVEN),
                        NumberPad(n),
                        ButtonStyle {
                            default: COL_CELL_GIVEN,
                            hover: COL_SAME_NUM,
                            pressed: COL_SELECTED,
                        },
                    ))
                    .with_children(|b| {
                        b.spawn((
                            Text::new(n.to_string()),
                            TextFont {
                                font_size: 24.0,
                                ..default()
                            },
                            TextColor(COL_TEXT_GIVEN),
                        ));
                    });
                }
                // Effacer
                pad.spawn((
                    Button,
                    Node {
                        width: Val::Px(72.0),
                        height: Val::Px(50.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border_radius: BorderRadius::all(Val::Px(10.0)),
                        margin: UiRect::left(Val::Px(6.0)),
                        ..default()
                    },
                    BackgroundColor(COL_TERRACOTTA),
                    NumberPad(0),
                    ButtonStyle {
                        default: COL_TERRACOTTA,
                        hover: Color::srgb(0.88, 0.55, 0.40),
                        pressed: Color::srgb(0.70, 0.38, 0.24),
                    },
                ))
                .with_children(|b| {
                    b.spawn((
                        Text::new("efface"),
                        TextFont {
                            font_size: 16.0,
                            ..default()
                        },
                        TextColor(COL_WHITE),
                    ));
                });
            });

            // Nouvelle partie
            root.spawn((
                Button,
                Node {
                    margin: UiRect::top(Val::Px(10.0)),
                    padding: UiRect::axes(Val::Px(22.0), Val::Px(10.0)),
                    border_radius: BorderRadius::all(Val::Px(12.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(COL_LEAF),
                NewGameButton,
                ButtonStyle {
                    default: COL_LEAF,
                    hover: Color::srgb(0.55, 0.72, 0.50),
                    pressed: COL_LEAF_DARK,
                },
            ))
            .with_children(|b| {
                b.spawn((
                    Text::new("nouvelle partie"),
                    TextFont {
                        font_size: 18.0,
                        ..default()
                    },
                    TextColor(COL_WHITE),
                ));
            });

            // Bannière de victoire (masquée tant qu'on n'a pas gagné)
            root.spawn((
                Node {
                    margin: UiRect::top(Val::Px(8.0)),
                    padding: UiRect::axes(Val::Px(24.0), Val::Px(10.0)),
                    border_radius: BorderRadius::all(Val::Px(12.0)),
                    display: Display::None,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(COL_TERRACOTTA),
                WinBanner,
            ))
            .with_children(|b| {
                b.spawn((
                    Text::new("Le jardin est en paix ~ bravo !"),
                    TextFont {
                        font_size: 20.0,
                        ..default()
                    },
                    TextColor(COL_WHITE),
                ));
            });
        });
}

fn spawn_leaf(
    commands: &mut Commands,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    color: Color,
    rotation: f32,
) {
    // Décor: ellipse approchée via Sprite teinté + rotation. Plein écran de 900x820
    // donc on convertit le coin haut-gauche en coordonnées centrées caméra2d.
    let cx = x + w * 0.5 - WIN_W as f32 * 0.5;
    let cy = WIN_H as f32 * 0.5 - (y + h * 0.5);
    commands.spawn((
        Sprite {
            color,
            custom_size: Some(Vec2::new(w, h)),
            ..default()
        },
        Transform::from_xyz(cx, cy, -1.0).with_rotation(Quat::from_rotation_z(rotation)),
    ));
}

// ---------- Interaction ----------

fn cell_click_system(
    mut q: Query<(&Interaction, &Cell), Changed<Interaction>>,
    mut writer: MessageWriter<SelectCell>,
) {
    for (i, cell) in &mut q {
        if *i == Interaction::Pressed {
            writer.write(SelectCell {
                row: cell.row,
                col: cell.col,
            });
        }
    }
}

fn number_pad_click_system(
    mut q: Query<(&Interaction, &NumberPad), Changed<Interaction>>,
    mut writer: MessageWriter<EnterNumber>,
) {
    for (i, np) in &mut q {
        if *i == Interaction::Pressed {
            writer.write(EnterNumber(np.0));
        }
    }
}

fn new_game_click_system(
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

fn keyboard_input_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut writer: MessageWriter<EnterNumber>,
    mut selection: ResMut<Selection>,
) {
    let pressed = [
        (KeyCode::Digit1, 1u8),
        (KeyCode::Digit2, 2),
        (KeyCode::Digit3, 3),
        (KeyCode::Digit4, 4),
        (KeyCode::Digit5, 5),
        (KeyCode::Digit6, 6),
        (KeyCode::Digit7, 7),
        (KeyCode::Digit8, 8),
        (KeyCode::Digit9, 9),
        (KeyCode::Numpad1, 1),
        (KeyCode::Numpad2, 2),
        (KeyCode::Numpad3, 3),
        (KeyCode::Numpad4, 4),
        (KeyCode::Numpad5, 5),
        (KeyCode::Numpad6, 6),
        (KeyCode::Numpad7, 7),
        (KeyCode::Numpad8, 8),
        (KeyCode::Numpad9, 9),
    ];
    for (kc, v) in pressed {
        if keys.just_pressed(kc) {
            writer.write(EnterNumber(v));
        }
    }
    if keys.just_pressed(KeyCode::Backspace)
        || keys.just_pressed(KeyCode::Delete)
        || keys.just_pressed(KeyCode::Digit0)
        || keys.just_pressed(KeyCode::Numpad0)
        || keys.just_pressed(KeyCode::Space)
    {
        writer.write(EnterNumber(0));
    }

    if let Some((r, c)) = selection.0 {
        let mut nr = r as i32;
        let mut nc = c as i32;
        if keys.just_pressed(KeyCode::ArrowUp) {
            nr -= 1;
        }
        if keys.just_pressed(KeyCode::ArrowDown) {
            nr += 1;
        }
        if keys.just_pressed(KeyCode::ArrowLeft) {
            nc -= 1;
        }
        if keys.just_pressed(KeyCode::ArrowRight) {
            nc += 1;
        }
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

fn handle_select_cell(mut reader: MessageReader<SelectCell>, mut selection: ResMut<Selection>) {
    for ev in reader.read() {
        selection.0 = Some((ev.row, ev.col));
    }
}

fn handle_enter_number(
    mut reader: MessageReader<EnterNumber>,
    selection: Res<Selection>,
    mut game: ResMut<SudokuGame>,
) {
    for ev in reader.read() {
        if game.won {
            continue;
        }
        let Some((r, c)) = selection.0 else { continue };
        let (r, c) = (r as usize, c as usize);
        if game.given[r][c] {
            continue;
        }
        game.current[r][c] = ev.0;
        if is_complete_and_valid(&game.current) {
            game.won = true;
        }
    }
}

// ---------- Rendu ----------

fn update_cell_visuals(
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
                **text = if value == 0 {
                    String::new()
                } else {
                    value.to_string()
                };
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

fn update_win_banner(game: Res<SudokuGame>, mut q: Query<&mut Node, With<WinBanner>>) {
    if !game.is_changed() {
        return;
    }
    for mut node in &mut q {
        node.display = if game.won {
            Display::Flex
        } else {
            Display::None
        };
    }
}

fn button_visual_feedback(
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
