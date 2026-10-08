use std::collections::HashMap;

use crate::database::Database;
use crate::{
    model::{
        quest_listener_param::QuestListenerParam, quest_param::QuestParam, todo_param::TodoParam,
    },
    utility::{self},
};
use anyhow::Result;
use rusqlite::Row;

pub struct Quest {
    pub number: i32,
    pub quest_id: String,
    pub name: String,
    pub genre: String,
    pub category: String,
    pub section: String,
    pub required_job: String,
    pub required_level: i32,
    pub listener_start: String,
    pub listener_end: String,
    pub required_quests: Vec<String>,
    pub quest_params: HashMap<String, i32>,
    pub quest_listeners: Vec<QuestListenerParam>,
    pub quest_todo: Vec<TodoParam>,
    pub quest_rewards: Vec<String>,
    pub quest_option_rewards: Vec<String>,
}

impl Quest {
    pub fn from_row(database: &Database, row: &Row) -> Result<Quest> {
        let required_job_int: i32 = row.get("ClassJobRequired")?;
        let mut required_job = String::new();
        if required_job_int != 0 {
            required_job = utility::head_all(
                database
                    .get_name_in_table("ClassJob", "NameEnglish", required_job_int)?
                    .unwrap_or(format!("#{required_job_int}"))
                    .as_str(),
            );
        }
        let listener_start_id: i32 = row.get("IssuerStart")?;
        let listener_end_id: i32 = row.get("TargetEnd")?;
        let mut param_map = HashMap::new();
        for index in 0..50 {
            let ia = QuestParam::from_row(row, index)?;
            if ia.instruction == "" {
                break;
            }
            param_map.insert(ia.instruction, ia.argument);
        }
        let mut listener_params = Vec::new();
        for index in 0..64 {
            let qp = QuestListenerParam::from_row(database, row, index)?;
            if qp.id == 0 {
                break;
            }
            if !(qp.actor_spawn_seq == 255 && qp.actor_despawn_seq == 255) {
                listener_params.push(qp);
            }
        }
        let mut todo_params = Vec::new();
        for index in 0..24 {
            let tp = TodoParam::from_row(database, row, index)?;
            if tp.location_ids.len() == 0 {
                break;
            }
            todo_params.push(tp);
        }
        let mut req_quest_list = Vec::new();
        for index in 0..3 {
            let quest_id: i32 = row.get(format!("PreviousQuest[{index}]").as_str())?;
            if quest_id == 0 {
                break;
            }
            let req_quest_name = database
                .get_name_in_table("Quest", "Name", quest_id)?
                .unwrap_or(format!("#{quest_id}"));
            if req_quest_name != "" {
                req_quest_list.push(format!("{req_quest_name} ({quest_id})"));
            }
        }
        let mut reward_list = Vec::new();
        for index in 0..3 {
            let catalyst_id = row.get(format!("ItemCatalyst[{index}]").as_str())?;
            if catalyst_id == 0 {
                break;
            }
            let catalyst_count = row.get(format!("ItemCountCatalyst[{index}]").as_str())?;
            if let Some(name) = database.get_item_qty(catalyst_id, catalyst_count, false, 0)? {
                reward_list.push(name);
            }
        }
        for index in 0..7 {
            let reward_id = row.get(format!("Reward[{index}]").as_str())?;
            if reward_id == 0 {
                break;
            }
            let reward_count = row.get(format!("ItemCountReward[{index}]").as_str())?;
            let reward_dye = row.get(format!("RewardStain[{index}]").as_str())?;
            if let Some(name) = database.get_item_qty(reward_id, reward_count, false, reward_dye)? {
                reward_list.push(name);
            }
        }
        let mut reward_optional_list = Vec::new();
        for index in 0..5 {
            let reward_id = row.get(format!("OptionalItemReward[{index}]").as_str())?;
            if reward_id == 0 {
                break;
            }
            let reward_count = row.get(format!("OptionalItemCountReward[{index}]").as_str())?;
            let reward_dye = row.get(format!("OptionalItemStainReward[{index}]").as_str())?;
            let is_hq = row.get(format!("OptionalItemIsHQReward[{index}]").as_str())?;
            if let Some(name) = database.get_item_qty(reward_id, reward_count, is_hq, reward_dye)? {
                reward_optional_list.push(name);
            }
        }
        Ok(Quest {
            number: row.get("RowId")?,
            quest_id: row.get("Id")?,
            name: row.get("Name")?,
            genre: row.get("GenreName").unwrap_or_default(),
            category: row.get("CategoryName").unwrap_or_default(),
            section: row.get("SectionName").unwrap_or_default(),
            required_job,
            required_level: row.get("ClassJobLevel[0]")?,
            listener_start: database
                .get_name_from_id(listener_start_id)?
                .unwrap_or(format!("#{listener_start_id}")),
            listener_end: database
                .get_name_from_id(listener_end_id)?
                .unwrap_or(format!("#{listener_end_id}")),
            required_quests: req_quest_list,
            quest_params: param_map,
            quest_listeners: listener_params,
            quest_todo: todo_params,
            quest_rewards: reward_list,
            quest_option_rewards: reward_optional_list,
        })
    }
}
