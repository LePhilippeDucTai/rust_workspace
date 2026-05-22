//! Mango — jeu éducatif Bevy. Phase 1 (MVP):
//! Boucle QCM, XP, amitié, cooldown 72h, sauvegarde JSON.

mod data;
mod gameplay;
mod raccoon;
mod save;
mod types;
mod ui;

use std::path::PathBuf;

use bevy::prelude::*;

use data::ProgressionData;
use gameplay::{FeedbackTimer, WaitTimer};
use save::{AutoSaveTimer, SaveData, SavePath};
use types::*;

const ASSETS_DIR: &str = "mango/assets";
const SAVE_FILE: &str = "mango/save/progression.json";

fn main() {
    let progression = match ProgressionData::load(ASSETS_DIR) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Erreur de chargement des données: {e}");
            return;
        }
    };

    let save_path = PathBuf::from(SAVE_FILE);
    let save_data = SaveData::load_or_default(&save_path);

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Mango — apprends en t'amusant".into(),
                resolution: (ui::WINDOW_WIDTH as u32, ui::WINDOW_HEIGHT as u32).into(),
                resizable: false,
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::srgb(0.03, 0.05, 0.08)))
        .insert_resource(progression)
        .insert_resource(save_data)
        .insert_resource(SavePath(save_path))
        .insert_resource(AutoSaveTimer(Timer::from_seconds(
            120.0,
            TimerMode::Repeating,
        )))
        .insert_resource(FeedbackTimer(Timer::from_seconds(0.0, TimerMode::Once)))
        .insert_resource(WaitTimer(Timer::from_seconds(1.5, TimerMode::Once)))
        .init_state::<GamePhase>()
        .add_message::<AnswerSelected>()
        .add_message::<CorrectAnswer>()
        .add_message::<IncorrectAnswer>()
        .add_message::<LevelUpEvent>()
        .add_systems(
            Startup,
            (ui::setup_scene, initialize_player_from_save).chain(),
        )
        .add_systems(Startup, jumpstart_state.after(initialize_player_from_save))
        .add_systems(
            Update,
            (
                gameplay::waiting_system,
                gameplay::handle_answer_selected,
                gameplay::process_correct,
                gameplay::process_incorrect,
                gameplay::detect_level_up_transition,
                gameplay::tick_feedback,
                gameplay::tick_level_up,
                ui::button_interaction_system,
                ui::rebuild_qcm_buttons,
                ui::clear_qcm_on_feedback,
                ui::update_hud,
                ui::update_dialogue,
                ui::update_feedback_text,
                save::auto_save_system,
                on_enter_waiting,
            ),
        )
        .run();
}

fn initialize_player_from_save(
    save: Res<SaveData>,
    mut player_q: Query<&mut Player>,
    mut raccoon_q: Query<&mut RaccoonState>,
    data: Res<ProgressionData>,
) {
    if let Ok(mut player) = player_q.single_mut() {
        player.level = save.player.current_level.max(1);
        player.friendship_points = save.player.friendship_points;
        player.xp_to_next_level = Player::xp_needed(player.level);
        player.current_xp = save.player.total_xp.saturating_sub(
            (0..player.level)
                .map(Player::xp_needed)
                .sum::<u32>()
                .saturating_sub(player.xp_to_next_level),
        );
        if player.current_xp > player.xp_to_next_level {
            player.current_xp = 0;
        }
    }
    if let Ok(mut raccoon) = raccoon_q.single_mut() {
        let player = player_q.single().ok();
        let level = player.map(|p| p.level).unwrap_or(1);
        let friendship = save.player.friendship_points;
        raccoon.emotion = EmotionState::Neutral;
        raccoon.current_dialogue =
            raccoon::pick_dialogue(&data, level, friendship, "on_game_start");
    }
}

fn jumpstart_state(mut next_state: ResMut<NextState<GamePhase>>) {
    next_state.set(GamePhase::WaitingForQuestion);
}

fn on_enter_waiting(state: Res<State<GamePhase>>, mut wait_timer: ResMut<WaitTimer>) {
    if state.is_changed() && *state.get() == GamePhase::WaitingForQuestion {
        wait_timer.0 = Timer::from_seconds(1.2, TimerMode::Once);
    }
}
