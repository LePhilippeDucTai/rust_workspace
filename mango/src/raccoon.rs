use crate::data::ProgressionData;
use crate::types::EmotionState;

pub fn pick_dialogue(
    data: &ProgressionData,
    level: u32,
    friendship: u32,
    context: &str,
) -> String {
    let mut best: Option<&crate::data::DialogueLine> = None;
    for line in &data.dialogues {
        if line.context != context {
            continue;
        }
        if line.level > level {
            continue;
        }
        if line.friendship_min > friendship {
            continue;
        }
        match best {
            None => best = Some(line),
            Some(b) => {
                let better = line.level > b.level
                    || (line.level == b.level && line.friendship_min > b.friendship_min);
                if better {
                    best = Some(line);
                }
            }
        }
    }
    best.map(|l| l.text.clone())
        .unwrap_or_else(|| "...".to_string())
}

pub fn emotion_color(emotion: EmotionState) -> bevy::prelude::Color {
    use bevy::prelude::Color;
    match emotion {
        EmotionState::Neutral => Color::srgb(1.0, 1.0, 1.0),
        EmotionState::Happy => Color::srgb(0.8, 1.0, 0.8),
        EmotionState::Confused => Color::srgb(1.0, 0.95, 0.8),
        EmotionState::Proud => Color::srgb(1.0, 0.85, 0.6),
        EmotionState::Sad => Color::srgb(0.75, 0.8, 0.95),
    }
}
