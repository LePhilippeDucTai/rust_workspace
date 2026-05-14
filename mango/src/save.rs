use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::types::{AnsweredQuestion, Theme};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PlayerSave {
    pub current_level: u32,
    pub total_xp: u32,
    pub friendship_points: u32,
    pub total_words_learned: u32,
}

#[derive(Resource, Debug, Clone, Serialize, Deserialize)]
pub struct SaveData {
    pub player: PlayerSave,
    pub answered_questions: Vec<AnsweredQuestion>,
    pub unlocked_themes: Vec<Theme>,
}

impl Default for SaveData {
    fn default() -> Self {
        Self {
            player: PlayerSave {
                current_level: 1,
                total_xp: 0,
                friendship_points: 0,
                total_words_learned: 0,
            },
            answered_questions: vec![],
            unlocked_themes: vec![Theme::Vocabulary],
        }
    }
}

#[derive(Resource)]
pub struct SavePath(pub PathBuf);

#[derive(Resource, Default)]
pub struct AutoSaveTimer(pub Timer);

impl SaveData {
    pub fn load_or_default(path: &PathBuf) -> Self {
        if let Ok(raw) = fs::read_to_string(path) {
            serde_json::from_str(&raw).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    pub fn write(&self, path: &PathBuf) {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(raw) = serde_json::to_string_pretty(self) {
            let _ = fs::write(path, raw);
        }
    }
}

pub fn auto_save_system(
    time: Res<Time>,
    mut timer: ResMut<AutoSaveTimer>,
    save_data: Res<SaveData>,
    save_path: Res<SavePath>,
) {
    if timer.0.tick(time.delta()).just_finished() {
        save_data.write(&save_path.0);
    }
}
