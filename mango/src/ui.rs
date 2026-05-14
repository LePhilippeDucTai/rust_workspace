use bevy::prelude::*;

use crate::types::*;

pub const WINDOW_WIDTH: f32 = 1280.0;
pub const WINDOW_HEIGHT: f32 = 800.0;

const PANEL_BG: Color = Color::srgba(0.05, 0.07, 0.12, 0.85);
const ACCENT: Color = Color::srgb(0.95, 0.78, 0.35);
const TEXT_LIGHT: Color = Color::srgb(0.98, 0.97, 0.95);
const QCM_BUTTON_BG: Color = Color::srgba(0.12, 0.15, 0.22, 0.92);
const QCM_BUTTON_HOVER: Color = Color::srgba(0.22, 0.27, 0.38, 0.95);
const QCM_BUTTON_PRESSED: Color = Color::srgba(0.32, 0.38, 0.50, 0.95);

#[derive(Component)]
pub struct QcmButtonsRoot;

pub fn setup_scene(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn(Camera2d);

    commands.spawn((
        Sprite {
            image: assets.load("cockpit-bg.jpg"),
            custom_size: Some(Vec2::new(WINDOW_WIDTH, WINDOW_HEIGHT)),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, -10.0),
    ));

    commands.spawn((
        Sprite {
            image: assets.load("mango-character.png"),
            custom_size: Some(Vec2::new(280.0, 280.0)),
            ..default()
        },
        Transform::from_xyz(-220.0, 40.0, 0.0),
        RaccoonSprite,
        RaccoonState {
            emotion: EmotionState::Neutral,
            current_dialogue: "...".into(),
        },
    ));

    commands.spawn((
        Sprite {
            image: assets.load("companion-bot.png"),
            custom_size: Some(Vec2::new(180.0, 180.0)),
            ..default()
        },
        Transform::from_xyz(280.0, 80.0, 0.0),
    ));

    commands.spawn(Player::new());

    spawn_hud(&mut commands);
    spawn_word_panel(&mut commands);
    spawn_qcm_root(&mut commands);
    spawn_dialogue_bubble(&mut commands);
    spawn_feedback_overlay(&mut commands);
}

fn spawn_hud(commands: &mut Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(12.0),
                left: Val::Px(12.0),
                right: Val::Px(12.0),
                padding: UiRect::all(Val::Px(10.0)),
                column_gap: Val::Px(20.0),
                align_items: AlignItems::Center,
                border_radius: BorderRadius::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(PANEL_BG),
        ))
        .with_children(|root| {
            root.spawn((
                Text::new(""),
                TextFont {
                    font_size: 18.0,
                    ..default()
                },
                TextColor(TEXT_LIGHT),
                StatusText,
            ));

            root.spawn((
                Node {
                    width: Val::Px(280.0),
                    height: Val::Px(18.0),
                    margin: UiRect::left(Val::Px(20.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.1, 0.12, 0.2)),
                BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.35)),
            ))
            .with_children(|bar| {
                bar.spawn((
                    Node {
                        width: Val::Percent(0.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.35, 0.75, 1.0)),
                    XpBarFill,
                ));
            });

            root.spawn((
                Node {
                    width: Val::Px(220.0),
                    height: Val::Px(18.0),
                    margin: UiRect::left(Val::Px(20.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.18, 0.1, 0.15)),
                BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.35)),
            ))
            .with_children(|bar| {
                bar.spawn((
                    Node {
                        width: Val::Percent(0.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(1.0, 0.42, 0.58)),
                    FriendshipBarFill,
                ));
            });
        });
}

fn spawn_word_panel(commands: &mut Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(80.0),
                left: Val::Percent(50.0),
                margin: UiRect::left(Val::Px(-180.0)),
                width: Val::Px(360.0),
                padding: UiRect::all(Val::Px(16.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border_radius: BorderRadius::all(Val::Px(12.0)),
                ..default()
            },
            BackgroundColor(PANEL_BG),
        ))
        .with_children(|p| {
            p.spawn((
                Text::new("..."),
                TextFont {
                    font_size: 44.0,
                    ..default()
                },
                TextColor(ACCENT),
                WordLabel,
            ));
        });
}

fn spawn_qcm_root(commands: &mut Commands) {
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(40.0),
            left: Val::Px(40.0),
            right: Val::Px(40.0),
            display: Display::Grid,
            grid_template_columns: vec![GridTrack::flex(1.0), GridTrack::flex(1.0)],
            grid_template_rows: vec![GridTrack::flex(1.0), GridTrack::flex(1.0)],
            row_gap: Val::Px(14.0),
            column_gap: Val::Px(14.0),
            height: Val::Px(220.0),
            ..default()
        },
        QcmContainer,
        QcmButtonsRoot,
    ));
}

fn spawn_dialogue_bubble(commands: &mut Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(140.0),
                left: Val::Px(40.0),
                width: Val::Px(280.0),
                padding: UiRect::all(Val::Px(12.0)),
                border_radius: BorderRadius::all(Val::Px(14.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.95, 0.95, 0.97, 0.92)),
        ))
        .with_children(|p| {
            p.spawn((
                Text::new("..."),
                TextFont {
                    font_size: 16.0,
                    ..default()
                },
                TextColor(Color::srgb(0.12, 0.13, 0.18)),
                DialogueBubble,
            ));
        });
}

fn spawn_feedback_overlay(commands: &mut Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(360.0),
                left: Val::Percent(50.0),
                margin: UiRect::left(Val::Px(-200.0)),
                width: Val::Px(400.0),
                padding: UiRect::all(Val::Px(10.0)),
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::NONE),
        ))
        .with_children(|p| {
            p.spawn((
                Text::new(""),
                TextFont {
                    font_size: 22.0,
                    ..default()
                },
                TextColor(TEXT_LIGHT),
                FeedbackText,
            ));
        });
}

pub fn rebuild_qcm_buttons(
    mut commands: Commands,
    container_q: Query<Entity, With<QcmContainer>>,
    question_q: Query<&CurrentQuestion, Added<CurrentQuestion>>,
    word_label_q: Query<Entity, With<WordLabel>>,
) {
    let Ok(question) = question_q.single() else {
        return;
    };
    let Ok(container) = container_q.single() else {
        return;
    };

    commands.entity(container).despawn_related::<Children>();

    commands.entity(container).with_children(|c| {
        for (i, option) in question.options.iter().enumerate() {
            c.spawn((
                Button,
                Node {
                    padding: UiRect::all(Val::Px(14.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border_radius: BorderRadius::all(Val::Px(12.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(QCM_BUTTON_BG),
                BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.25)),
                QcmButton { index: i },
            ))
            .with_children(|b| {
                b.spawn((
                    Text::new(option.clone()),
                    TextFont {
                        font_size: 16.0,
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 1.0, 1.0)),
                    TextLayout::new_with_justify(Justify::Center),
                ));
            });
        }
    });

    for ent in &word_label_q {
        commands
            .entity(ent)
            .insert(Text::new(question.word.clone()));
    }
}

pub fn clear_qcm_on_feedback(
    mut commands: Commands,
    state: Res<State<GamePhase>>,
    container_q: Query<Entity, With<QcmContainer>>,
    word_label_q: Query<Entity, With<WordLabel>>,
) {
    if !state.is_changed() {
        return;
    }
    if *state.get() != GamePhase::FeedbackShowing && *state.get() != GamePhase::WaitingForQuestion
    {
        return;
    }
    for ent in &container_q {
        commands.entity(ent).despawn_related::<Children>();
    }
    if *state.get() == GamePhase::WaitingForQuestion {
        for ent in &word_label_q {
            commands.entity(ent).insert(Text::new("..."));
        }
    }
}

pub fn button_interaction_system(
    mut interactions: Query<
        (&Interaction, &QcmButton, &mut BackgroundColor),
        Changed<Interaction>,
    >,
    mut events: MessageWriter<AnswerSelected>,
) {
    for (interaction, button, mut bg) in &mut interactions {
        match *interaction {
            Interaction::Pressed => {
                bg.0 = QCM_BUTTON_PRESSED;
                events.write(AnswerSelected {
                    button_index: button.index,
                });
            }
            Interaction::Hovered => {
                bg.0 = QCM_BUTTON_HOVER;
            }
            Interaction::None => {
                bg.0 = QCM_BUTTON_BG;
            }
        }
    }
}

pub fn update_hud(
    player_q: Query<&Player, Changed<Player>>,
    mut status_q: Query<&mut Text, With<StatusText>>,
    mut xp_q: Query<&mut Node, (With<XpBarFill>, Without<FriendshipBarFill>)>,
    mut friendship_q: Query<&mut Node, (With<FriendshipBarFill>, Without<XpBarFill>)>,
) {
    let Ok(player) = player_q.single() else {
        return;
    };
    if let Ok(mut text) = status_q.single_mut() {
        **text = format!(
            "Niveau {}   XP {}/{}   Amitié {}/100",
            player.level, player.current_xp, player.xp_to_next_level, player.friendship_points
        );
    }
    let xp_pct = (player.current_xp as f32 / player.xp_to_next_level.max(1) as f32).clamp(0.0, 1.0);
    if let Ok(mut node) = xp_q.single_mut() {
        node.width = Val::Percent(xp_pct * 100.0);
    }
    let f_pct = (player.friendship_points as f32 / 100.0).clamp(0.0, 1.0);
    if let Ok(mut node) = friendship_q.single_mut() {
        node.width = Val::Percent(f_pct * 100.0);
    }
}

pub fn update_dialogue(
    raccoon_q: Query<&RaccoonState, Changed<RaccoonState>>,
    mut text_q: Query<&mut Text, With<DialogueBubble>>,
) {
    let Ok(raccoon) = raccoon_q.single() else {
        return;
    };
    if let Ok(mut text) = text_q.single_mut() {
        **text = raccoon.current_dialogue.clone();
    }
}

pub fn update_feedback_text(
    state: Res<State<GamePhase>>,
    mut text_q: Query<(&mut Text, &mut TextColor), With<FeedbackText>>,
    correct_evt: MessageReader<CorrectAnswer>,
    wrong_evt: MessageReader<IncorrectAnswer>,
) {
    let Ok((mut text, mut color)) = text_q.single_mut() else {
        return;
    };
    if !correct_evt.is_empty() {
        **text = "Bonne réponse ! +10 XP".to_string();
        color.0 = Color::srgb(0.55, 1.0, 0.65);
    } else if !wrong_evt.is_empty() {
        **text = "Pas vraiment... Réessaie dans 72h !".to_string();
        color.0 = Color::srgb(1.0, 0.7, 0.3);
    } else if state.is_changed() && *state.get() == GamePhase::WaitingForQuestion {
        **text = String::new();
    } else if state.is_changed() && *state.get() == GamePhase::LevelUp {
        **text = "LEVEL UP !".to_string();
        color.0 = Color::srgb(1.0, 0.9, 0.4);
    }
}
