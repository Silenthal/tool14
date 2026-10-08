use anyhow::Result;
use rusqlite::Row;

use crate::database::Database;

#[derive(Default, Debug, PartialEq, Eq)]
pub struct TodoParam {
    pub location_ids: Vec<String>,
    pub todo_complete_seq: i32,
    pub todo_qty: i32,
    pub countable_num: i32,
}

impl TodoParam {
    pub fn from_row(database: &Database, row: &Row, index: i32) -> Result<TodoParam> {
        let prefix = format!("TodoParams[{index}].");
        let mut location_list = Vec::new();
        for i in 0..8 {
            let location: i32 = row.get(format!("{prefix}ToDoLocation[{i}]").as_str())?;
            if location == 0 {
                break;
            }
            let todo_s = database
                .get_level_ref(location)?
                .unwrap_or(location.to_string());
            location_list.push(todo_s);
        }
        Ok(TodoParam {
            location_ids: location_list,
            todo_complete_seq: row.get(format!("{prefix}ToDoCompleteSeq").as_str())?,
            todo_qty: row.get(format!("{prefix}ToDoQty").as_str())?,
            countable_num: row.get(format!("{prefix}CountableNum").as_str())?,
        })
    }
}
