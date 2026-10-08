use anyhow::Result;
use rusqlite::Row;

#[derive(Default, Debug, PartialEq, Eq)]
pub struct QuestParam {
    pub instruction: String,
    pub argument: i32,
}

impl QuestParam {
    pub fn from_row(row: &Row, index: i32) -> Result<QuestParam> {
        let prefix = format!("QuestParams[{index}].");
        Ok(QuestParam {
            instruction: row.get(format!("{prefix}ScriptInstruction").as_str())?,
            argument: row.get(format!("{prefix}ScriptArg").as_str())?,
        })
    }
}
