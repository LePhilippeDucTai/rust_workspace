use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;

use crate::types::Theme;

#[derive(Debug, Clone, Deserialize)]
pub struct Word {
    pub id: String,
    pub word: String,
    pub level_unlock: u32,
    pub theme: Theme,
    #[allow(dead_code)]
    pub difficulty: u32,
    #[allow(dead_code)]
    pub definition: String,
    #[allow(dead_code)]
    pub sentence_context: String,
    #[allow(dead_code)]
    pub learning_tip: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QcmData {
    pub word_id: String,
    pub correct_answer: String,
    pub wrong_answers: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DialogueLine {
    pub level: u32,
    pub friendship_min: u32,
    pub context: String,
    pub text: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ThemeMeta {
    pub id: String,
    pub name: String,
    pub unlock_level: u32,
    pub color_hex: String,
}

#[derive(Deserialize)]
struct WordsFile {
    words: Vec<Word>,
}

#[derive(Deserialize)]
struct QcmFile {
    qcm_choices: Vec<QcmData>,
}

#[derive(Deserialize)]
struct DialogueFile {
    raccoon_dialogues: Vec<DialogueLine>,
}

#[derive(Deserialize)]
struct ThemesFile {
    themes: Vec<ThemeMeta>,
}

#[derive(Resource)]
pub struct ProgressionData {
    pub words: Vec<Word>,
    pub qcm: HashMap<String, QcmData>,
    pub dialogues: Vec<DialogueLine>,
    #[allow(dead_code)]
    pub themes: Vec<ThemeMeta>,
}

impl ProgressionData {
    pub fn load(assets_dir: &str) -> Result<Self, String> {
        let words: WordsFile = read_json(&format!("{assets_dir}/data/mots.json"))?;
        let qcm: QcmFile = read_json(&format!("{assets_dir}/data/definitions.json"))?;
        let dialogues: DialogueFile = read_json(&format!("{assets_dir}/data/dialogue.json"))?;
        let themes: ThemesFile = read_json(&format!("{assets_dir}/data/themes.json"))?;
        let qcm_map: HashMap<String, QcmData> = qcm
            .qcm_choices
            .into_iter()
            .map(|q| (q.word_id.clone(), q))
            .collect();
        Ok(Self {
            words: words.words,
            qcm: qcm_map,
            dialogues: dialogues.raccoon_dialogues,
            themes: themes.themes,
        })
    }
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &str) -> Result<T, String> {
    let raw = fs::read_to_string(path).map_err(|e| format!("read {path}: {e}"))?;
    serde_json::from_str(&raw).map_err(|e| format!("parse {path}: {e}"))
}
