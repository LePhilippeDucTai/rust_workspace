use crate::data::ProgressionData;

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

