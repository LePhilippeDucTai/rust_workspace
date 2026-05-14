use bevy::prelude::*;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, States, Default)]
pub enum GamePhase {
    #[default]
    Loading,
    WaitingForQuestion,
    QuestionDisplayed,
    FeedbackShowing,
    LevelUp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Theme {
    #[serde(rename = "VOCABULARY")]
    Vocabulary,
    #[serde(rename = "MATHEMATICS")]
    Mathematics,
    #[serde(rename = "SCIENCES")]
    Sciences,
    #[serde(rename = "HISTORY")]
    History,
    #[serde(rename = "CULTURE")]
    Culture,
}

impl Theme {
    pub fn unlock_level(self) -> u32 {
        match self {
            Theme::Vocabulary => 1,
            Theme::Mathematics => 6,
            Theme::Sciences => 11,
            Theme::History => 16,
            Theme::Culture => 16,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmotionState {
    Neutral,
    Happy,
    Confused,
    #[allow(dead_code)]
    Proud,
    Sad,
}

#[derive(Component)]
pub struct Player {
    pub level: u32,
    pub current_xp: u32,
    pub xp_to_next_level: u32,
    pub friendship_points: u32,
}

impl Player {
    pub fn xp_needed(level: u32) -> u32 {
        100 * level
    }
    pub fn new() -> Self {
        Self {
            level: 1,
            current_xp: 0,
            xp_to_next_level: Self::xp_needed(1),
            friendship_points: 0,
        }
    }
}

#[derive(Component)]
pub struct RaccoonState {
    pub emotion: EmotionState,
    pub current_dialogue: String,
}

#[derive(Component, Debug, Clone)]
pub struct CurrentQuestion {
    pub word_id: String,
    pub word: String,
    pub correct_index: usize,
    pub options: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnsweredQuestion {
    pub word_id: String,
    pub answered_at: DateTime<Utc>,
    pub was_correct: bool,
    pub cooldown_until: Option<DateTime<Utc>>,
}

#[derive(Component, Clone, Copy)]
pub struct QcmButton {
    pub index: usize,
}

#[derive(Component)]
pub struct WordLabel;

#[derive(Component)]
pub struct DialogueBubble;

#[derive(Component)]
pub struct XpBarFill;

#[derive(Component)]
pub struct FriendshipBarFill;

#[derive(Component)]
pub struct StatusText;

#[derive(Component)]
pub struct FeedbackText;

#[derive(Component)]
pub struct RaccoonSprite;

#[derive(Component)]
pub struct QcmContainer;

#[derive(Message)]
pub struct AnswerSelected {
    pub button_index: usize,
}

#[derive(Message)]
pub struct CorrectAnswer;

#[derive(Message)]
pub struct IncorrectAnswer;

#[derive(Message)]
pub struct LevelUpEvent {
    #[allow(dead_code)]
    pub new_level: u32,
}
