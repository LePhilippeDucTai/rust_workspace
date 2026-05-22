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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_game_phase_default() {
        assert_eq!(GamePhase::default(), GamePhase::Loading);
    }

    #[test]
    fn test_theme_unlock_levels() {
        assert_eq!(Theme::Vocabulary.unlock_level(), 1);
        assert_eq!(Theme::Mathematics.unlock_level(), 6);
        assert_eq!(Theme::Sciences.unlock_level(), 11);
        assert_eq!(Theme::History.unlock_level(), 16);
        assert_eq!(Theme::Culture.unlock_level(), 16);
    }

    #[test]
    fn test_emotion_states_exist() {
        let emotions = vec![
            EmotionState::Neutral,
            EmotionState::Happy,
            EmotionState::Confused,
            EmotionState::Proud,
            EmotionState::Sad,
        ];
        assert_eq!(emotions.len(), 5);
    }

    #[test]
    fn test_player_creation() {
        let player = Player::new();
        assert_eq!(player.level, 1);
        assert_eq!(player.current_xp, 0);
        assert_eq!(player.xp_to_next_level, Player::xp_needed(1));
        assert_eq!(player.friendship_points, 0);
    }

    #[test]
    fn test_player_xp_needed() {
        assert_eq!(Player::xp_needed(1), 100);
        assert_eq!(Player::xp_needed(2), 200);
        assert_eq!(Player::xp_needed(5), 500);
        assert_eq!(Player::xp_needed(10), 1000);
    }

    #[test]
    fn test_player_xp_requirement_increases_with_level() {
        let xp1 = Player::xp_needed(1);
        let xp10 = Player::xp_needed(10);
        assert!(xp10 > xp1);
    }

    #[test]
    fn test_raccoon_state_creation() {
        let raccoon = RaccoonState {
            emotion: EmotionState::Happy,
            current_dialogue: "Hello!".to_string(),
        };
        assert_eq!(raccoon.emotion, EmotionState::Happy);
        assert_eq!(raccoon.current_dialogue, "Hello!");
    }

    #[test]
    fn test_current_question_structure() {
        let question = CurrentQuestion {
            word_id: "word1".to_string(),
            word: "hello".to_string(),
            correct_index: 1,
            options: vec!["hi".to_string(), "hello".to_string(), "hey".to_string()],
        };
        assert_eq!(question.word, "hello");
        assert_eq!(question.correct_index, 1);
        assert_eq!(question.options.len(), 3);
    }

    #[test]
    fn test_qcm_button() {
        let btn1 = QcmButton { index: 0 };
        let btn2 = QcmButton { index: 1 };
        assert_eq!(btn1.index, 0);
        assert_eq!(btn2.index, 1);
    }
}
