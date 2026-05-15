use bevy::prelude::*;

use crate::colors::*;
use crate::components::*;

pub fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font = asset_server.load("fonts/Geneva.ttf");
    commands.spawn(Camera2d);

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
                TextFont { font_size: 38.0, font: font.clone(), ..default() },
                TextColor(COL_TITLE),
            ));
            root.spawn((
                Text::new("~ une promenade chiffrée ~"),
                TextFont { font_size: 15.0, font: font.clone(), ..default() },
                TextColor(COL_SUBTITLE),
            ));

            // Plateau 3×3 blocs
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
                                                        font: font.clone(),
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
                            TextFont { font_size: 24.0, font: font.clone(), ..default() },
                            TextColor(COL_TEXT_GIVEN),
                        ));
                    });
                }
                // Bouton effacer
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
                        TextFont { font_size: 16.0, font: font.clone(), ..default() },
                        TextColor(COL_WHITE),
                    ));
                });
            });

            // Boutons annuler / refaire
            root.spawn(Node {
                display: Display::Flex,
                column_gap: Val::Px(8.0),
                margin: UiRect::top(Val::Px(8.0)),
                ..default()
            })
            .with_children(|row| {
                row.spawn((
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(18.0), Val::Px(9.0)),
                        border_radius: BorderRadius::all(Val::Px(10.0)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(COL_SUBTITLE),
                    UndoButton,
                    ButtonStyle {
                        default: COL_SUBTITLE,
                        hover: Color::srgb(0.65, 0.50, 0.38),
                        pressed: Color::srgb(0.40, 0.28, 0.18),
                    },
                ))
                .with_children(|b| {
                    b.spawn((
                        Text::new("← annuler"),
                        TextFont { font_size: 16.0, font: font.clone(), ..default() },
                        TextColor(COL_WHITE),
                    ));
                });

                row.spawn((
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(18.0), Val::Px(9.0)),
                        border_radius: BorderRadius::all(Val::Px(10.0)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(COL_SUBTITLE),
                    RedoButton,
                    ButtonStyle {
                        default: COL_SUBTITLE,
                        hover: Color::srgb(0.65, 0.50, 0.38),
                        pressed: Color::srgb(0.40, 0.28, 0.18),
                    },
                ))
                .with_children(|b| {
                    b.spawn((
                        Text::new("refaire →"),
                        TextFont { font_size: 16.0, font: font.clone(), ..default() },
                        TextColor(COL_WHITE),
                    ));
                });
            });

            // Bouton nouvelle partie
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
                    TextFont { font_size: 18.0, font: font.clone(), ..default() },
                    TextColor(COL_WHITE),
                ));
            });

            // Bannière de victoire (masquée par défaut)
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
                    TextFont { font_size: 20.0, font: font.clone(), ..default() },
                    TextColor(COL_WHITE),
                ));
            });
        });
}

pub fn spawn_leaf(
    commands: &mut Commands,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    color: Color,
    rotation: f32,
) {
    let cx = x + w * 0.5 - WIN_W as f32 * 0.5;
    let cy = WIN_H as f32 * 0.5 - (y + h * 0.5);
    commands.spawn((
        Sprite { color, custom_size: Some(Vec2::new(w, h)), ..default() },
        Transform::from_xyz(cx, cy, -1.0).with_rotation(Quat::from_rotation_z(rotation)),
    ));
}
