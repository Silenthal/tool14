use anyhow::Result;
use rusqlite::Row;
use std::{collections::HashMap, fmt};

#[derive(Default, Debug, PartialEq, Eq, Clone)]
pub struct QuestBattle {
    pub quest_link: i32,
    pub time_limit: i32,
    pub level_sync: i32,
    pub battle_scene: i32,
    pub quest_battle_params: HashMap<String, i32>,
}

impl QuestBattle {
    pub fn from_row(row: &Row) -> Result<QuestBattle> {
        let quest_link: i32 = row.get("Quest")?;
        let time_limit: i32 = row.get("TimeLimit")?;
        let level_sync: i32 = row.get("LevelSync")?;
        let battle_scene: i32 = row.get("QuestBattleScene")?;
        let mut param_map = HashMap::new();
        let qbp_prefix = "QuestBattleParams[";
        for index in 0..220 {
            let instruction =
                row.get(format!("{qbp_prefix}{index}].ScriptInstruction").as_str())?;
            if instruction == "" {
                break;
            }
            let argument = row.get(format!("{qbp_prefix}{index}].ScriptValue").as_str())?;
            param_map.insert(instruction, argument);
        }
        Ok(QuestBattle {
            quest_link,
            time_limit,
            level_sync,
            battle_scene,
            quest_battle_params: param_map,
        })
    }
}

impl fmt::Display for QuestBattle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut out = Vec::new();
        for (k, v) in &self.quest_battle_params {
            out.push(format!("{k}: {v}"));
        }
        write!(f, "{}", out.join("\n"))
    }
}
