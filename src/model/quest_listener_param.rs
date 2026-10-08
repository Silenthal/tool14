use std::fmt;

use anyhow::Result;
use rusqlite::Row;

use crate::database::Database;

#[derive(Default, Debug, PartialEq, Eq)]
pub struct QuestListenerParam {
    pub display: String,
    pub id: i32,
    pub behavior: i32,
    pub actor_spawn_seq: i32,
    pub actor_despawn_seq: i32,
    pub quest_uint_8a: i32,
    pub condition_value: i32,
    pub condition_type: i32,
    // Looking over all quests, this always seems to be the value 3
    // pub condition_operator: i32,
    pub condition_bool: bool,
    pub visible_bool: bool,
    pub item_bool: bool,
    pub announce_bool: bool,
    pub behavior_bool: bool,
    pub accept_bool: bool,
    pub qualified_bool: bool,
    pub can_target_bool: bool,
}

impl QuestListenerParam {
    pub fn from_row(database: &Database, row: &Row, index: i32) -> Result<QuestListenerParam> {
        let prefix = format!("QuestListenerParams[{index}].");
        let lookup = row.get(format!("{prefix}Listener").as_str())?;
        Ok(QuestListenerParam {
            id: lookup,
            display: database.get_name_from_id(lookup)?.unwrap_or_default(),
            behavior: row.get(format!("{prefix}Behavior").as_str())?,
            actor_spawn_seq: row.get(format!("{prefix}ActorSpawnSeq").as_str())?,
            actor_despawn_seq: row.get(format!("{prefix}ActorDespawnSeq").as_str())?,
            quest_uint_8a: row.get(format!("{prefix}QuestUInt8A").as_str())?,
            condition_value: row.get(format!("{prefix}ConditionValue").as_str())?,
            condition_type: row.get(format!("{prefix}ConditionType").as_str())?,
            // condition_operator: row.get(format!("{prefix}ConditionOperator").as_str())?,
            condition_bool: row.get(format!("{prefix}ConditionBool").as_str())?,
            visible_bool: row.get(format!("{prefix}VisibleBool").as_str())?,
            item_bool: row.get(format!("{prefix}ItemBool").as_str())?,
            announce_bool: row.get(format!("{prefix}AnnounceBool").as_str())?,
            behavior_bool: row.get(format!("{prefix}BehaviorBool").as_str())?,
            accept_bool: row.get(format!("{prefix}AcceptBool").as_str())?,
            qualified_bool: row.get(format!("{prefix}QualifiedBool").as_str())?,
            can_target_bool: row.get(format!("{prefix}CanTargetBool").as_str())?,
        })
    }
}

impl fmt::Display for QuestListenerParam {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name: String;
        if self.display != "" {
            name = self.display.clone();
        } else {
            name = format!("#{}", self.id);
        }
        let mut tags = Vec::new();
        if self.visible_bool {
            tags.push("visible")
        }
        if self.item_bool {
            tags.push("item")
        }
        if self.announce_bool {
            tags.push("announce")
        }
        if self.behavior_bool {
            tags.push("behavior");
        }
        if self.accept_bool {
            tags.push("accept");
        }
        if self.qualified_bool {
            tags.push("qualified");
        }
        if self.can_target_bool {
            tags.push("targetable");
        }
        write!(
            f,
            "QL: {} {{id: {}, behavior: {}, spawn_seq: {}, despawn_seq: {}, uint8a: {}, cond_val: {}, cond_type: {}, cond_bool: {}, tags: \"{}\"}}",
            name,
            self.id,
            self.behavior,
            self.actor_spawn_seq,
            self.actor_despawn_seq,
            self.quest_uint_8a,
            self.condition_value,
            self.condition_type,
            self.condition_bool,
            tags.join(", ")
        )
    }
}
