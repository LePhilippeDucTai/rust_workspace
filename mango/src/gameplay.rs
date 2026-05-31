use bevy::prelude::*;
use chrono::{Duration, Utc};
use rand::seq::SliceRandom;
use rand::thread_rng;

use crate::data::ProgressionData;
use crate::raccoon::pick_dialogue;
use crate::save::SaveData;
use crate::types::*;

pub const COOLDOWN_HOURS: i64 = 72;
pub const XP_CORRECT: u32 = 10;
pub const XP_PENALTY_LVL3: u32 = 5;
pub const FRIENDSHIP_CORRECT: u32 = 2;
pub const FRIENDSHIP_WRONG_EMPATHY: u32 = 1;

#[derive(Resource, Default)]
pub struct FeedbackTimer(pub Timer);

#[derive(Resource, Default)]
pub struct WaitTimer(pub Timer);

pub fn pick_next_question(
    data: &ProgressionData,
    save: &SaveData,
    player_level: u32,
) -> Option<CurrentQuestion> {
    let now = Utc::now();
    let mut candidates: Vec<&crate::data::Word> = data
        .words
        .iter()
        .filter(|w| w.level_unlock <= player_level)
        .filter(|w| save.unlocked_themes.contains(&w.theme))
        .filter(|w| {
            let answered = save.answered_questions.iter().find(|a| a.word_id == w.id);
            match answered {
                None => true,
                Some(a) => match a.cooldown_until {
                    Some(until) => until <= now,
                    None => false,
                },
            }
        })
        .collect();

    let mut rng = thread_rng();
    candidates.shuffle(&mut rng);
    let word = candidates.first().copied()?;
    let qcm = data.qcm.get(&word.id)?;

    let mut options: Vec<String> = std::iter::once(qcm.correct_answer.clone())
        .chain(qcm.wrong_answers.iter().cloned())
        .collect();
    let correct = qcm.correct_answer.clone();
    options.shuffle(&mut rng);
    let correct_index = options.iter().position(|o| o == &correct).unwrap_or(0);

    Some(CurrentQuestion {
        word_id: word.id.clone(),
        word: word.word.clone(),
        correct_index,
        options,
    })
}

pub fn handle_answer_selected(
    mut events: MessageReader<AnswerSelected>,
    question_q: Query<&CurrentQuestion>,
    mut correct_w: MessageWriter<CorrectAnswer>,
    mut wrong_w: MessageWriter<IncorrectAnswer>,
    mut next_state: ResMut<NextState<GamePhase>>,
    state: Res<State<GamePhase>>,
) {
    if *state.get() != GamePhase::QuestionDisplayed {
        events.clear();
        return;
    }
    let Ok(question) = question_q.single() else {
        events.clear();
        return;
    };
    for ev in events.read() {
        if ev.button_index == question.correct_index {
            correct_w.write(CorrectAnswer);
        } else {
            wrong_w.write(IncorrectAnswer);
        }
        next_state.set(GamePhase::FeedbackShowing);
        break;
    }
}

pub fn process_correct(
    mut events: MessageReader<CorrectAnswer>,
    mut player_q: Query<&mut Player>,
    mut raccoon_q: Query<&mut RaccoonState>,
    question_q: Query<&CurrentQuestion>,
    data: Res<ProgressionData>,
    mut save: ResMut<SaveData>,
    mut feedback_timer: ResMut<FeedbackTimer>,
    mut level_up_w: MessageWriter<LevelUpEvent>,
) {
    if events.is_empty() {
        return;
    }
    events.clear();
    let Ok(mut player) = player_q.single_mut() else {
        return;
    };
    let Ok(mut raccoon) = raccoon_q.single_mut() else {
        return;
    };
    let Ok(question) = question_q.single() else {
        return;
    };

    player.current_xp += XP_CORRECT;
    player.friendship_points = (player.friendship_points + FRIENDSHIP_CORRECT).min(100);
    save.player.total_xp += XP_CORRECT;
    save.player.friendship_points = player.friendship_points;
    save.player.total_words_learned += 1;
    save.answered_questions.push(AnsweredQuestion {
        word_id: question.word_id.clone(),
        answered_at: Utc::now(),
        was_correct: true,
        cooldown_until: None,
    });

    if player.current_xp >= player.xp_to_next_level {
        let overflow = player.current_xp - player.xp_to_next_level;
        player.level += 1;
        player.current_xp = overflow;
        player.xp_to_next_level = Player::xp_needed(player.level);
        save.player.current_level = player.level;
        unlock_themes_for_level(&mut save, player.level);
        level_up_w.write(LevelUpEvent {
            new_level: player.level,
        });
    }

    raccoon.emotion = EmotionState::Happy;
    raccoon.current_dialogue = pick_dialogue(
        &data,
        player.level,
        player.friendship_points,
        "on_correct_answer",
    );

    feedback_timer.0 = Timer::from_seconds(2.5, TimerMode::Once);
}

pub fn process_incorrect(
    mut events: MessageReader<IncorrectAnswer>,
    mut player_q: Query<&mut Player>,
    mut raccoon_q: Query<&mut RaccoonState>,
    question_q: Query<&CurrentQuestion>,
    data: Res<ProgressionData>,
    mut save: ResMut<SaveData>,
    mut feedback_timer: ResMut<FeedbackTimer>,
) {
    if events.is_empty() {
        return;
    }
    events.clear();
    let Ok(mut player) = player_q.single_mut() else {
        return;
    };
    let Ok(mut raccoon) = raccoon_q.single_mut() else {
        return;
    };
    let Ok(question) = question_q.single() else {
        return;
    };

    let now = Utc::now();
    if player.level >= 3 {
        player.current_xp = player.current_xp.saturating_sub(XP_PENALTY_LVL3);
        save.player.total_xp = save.player.total_xp.saturating_sub(XP_PENALTY_LVL3);
    } else {
        player.friendship_points = (player.friendship_points + FRIENDSHIP_WRONG_EMPATHY).min(100);
        save.player.friendship_points = player.friendship_points;
    }
    save.answered_questions.push(AnsweredQuestion {
        word_id: question.word_id.clone(),
        answered_at: now,
        was_correct: false,
        cooldown_until: Some(now + Duration::hours(COOLDOWN_HOURS)),
    });

    raccoon.emotion = EmotionState::Sad;
    raccoon.current_dialogue = pick_dialogue(
        &data,
        player.level,
        player.friendship_points,
        "on_wrong_answer",
    );

    feedback_timer.0 = Timer::from_seconds(2.5, TimerMode::Once);
}

fn unlock_themes_for_level(save: &mut SaveData, level: u32) {
    for theme in [
        Theme::Vocabulary,
        Theme::Mathematics,
        Theme::Sciences,
        Theme::History,
        Theme::Culture,
    ] {
        if level >= theme.unlock_level() && !save.unlocked_themes.contains(&theme) {
            save.unlocked_themes.push(theme);
        }
    }
}

pub fn tick_feedback(
    time: Res<Time>,
    mut timer: ResMut<FeedbackTimer>,
    state: Res<State<GamePhase>>,
    mut next_state: ResMut<NextState<GamePhase>>,
    save: Res<SaveData>,
    save_path: Res<crate::save::SavePath>,
) {
    if *state.get() != GamePhase::FeedbackShowing {
        return;
    }
    if timer.0.duration().is_zero() {
        return;
    }
    if timer.0.tick(time.delta()).just_finished() {
        save.write(&save_path.0);
        next_state.set(GamePhase::WaitingForQuestion);
    }
}

pub fn tick_level_up(
    time: Res<Time>,
    mut timer: ResMut<FeedbackTimer>,
    state: Res<State<GamePhase>>,
    mut next_state: ResMut<NextState<GamePhase>>,
) {
    if *state.get() != GamePhase::LevelUp {
        return;
    }
    if timer.0.tick(time.delta()).just_finished() {
        next_state.set(GamePhase::WaitingForQuestion);
    }
}

pub fn detect_level_up_transition(
    mut events: MessageReader<LevelUpEvent>,
    mut next_state: ResMut<NextState<GamePhase>>,
    mut feedback_timer: ResMut<FeedbackTimer>,
) {
    if events.is_empty() {
        return;
    }
    events.clear();
    feedback_timer.0 = Timer::from_seconds(3.5, TimerMode::Once);
    next_state.set(GamePhase::LevelUp);
}

pub fn waiting_system(
    time: Res<Time>,
    mut wait_timer: ResMut<WaitTimer>,
    state: Res<State<GamePhase>>,
    mut next_state: ResMut<NextState<GamePhase>>,
    mut commands: Commands,
    question_q: Query<Entity, With<CurrentQuestion>>,
    data: Res<ProgressionData>,
    save: Res<SaveData>,
    player_q: Query<&Player>,
    mut raccoon_q: Query<&mut RaccoonState>,
) {
    if *state.get() != GamePhase::WaitingForQuestion {
        return;
    }
    if !wait_timer.0.tick(time.delta()).just_finished() && !wait_timer.0.is_finished() {
        return;
    }
    let Ok(player) = player_q.single() else {
        return;
    };
    for ent in &question_q {
        commands.entity(ent).despawn();
    }
    match pick_next_question(&data, &save, player.level) {
        Some(q) => {
            if let Ok(mut raccoon) = raccoon_q.single_mut() {
                raccoon.emotion = EmotionState::Confused;
                raccoon.current_dialogue =
                    pick_dialogue(&data, player.level, player.friendship_points, "on_question");
            }
            commands.spawn(q);
            next_state.set(GamePhase::QuestionDisplayed);
        }
        None => {
            if let Ok(mut raccoon) = raccoon_q.single_mut() {
                raccoon.emotion = EmotionState::Neutral;
                raccoon.current_dialogue =
                    "Reviens plus tard ! On a tout vu pour aujourd'hui.".to_string();
            }
            wait_timer.0 = Timer::from_seconds(5.0, TimerMode::Once);
        }
    }
}
