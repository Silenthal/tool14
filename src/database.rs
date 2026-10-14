use std::sync::LazyLock;
use std::{collections::HashMap, fs, path::PathBuf};

use crate::database_import;
use crate::settings;
use anyhow::{Context, Result, anyhow};
use ironworks::excel::{Excel, Field};
use regex::Regex;
use rusqlite::{Connection, params};

use crate::{
    cgwiki::formatter::format_string, model::quest_battle::QuestBattle, utility::head_all,
};

use crate::model::quest::Quest;

static REGEX_OWNER_CHECK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[A-Z]\w+'s? ").unwrap());

pub struct Database {
    pub conn: Connection,
    pub quest_sys_define_map: HashMap<String, i32>,
}

impl Database {
    pub fn new(connection: Connection, defines: HashMap<String, i32>) -> Self {
        Self {
            conn: connection,
            quest_sys_define_map: defines,
        }
    }

    pub fn create(excel: &Excel, force_update: bool) -> Result<Database> {
        let p: PathBuf = settings::get_cache_path()?;
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Unable to create cache directory at {:?}", parent))?;
        }
        let mut conn = Connection::open(&p)
            .with_context(|| format!("Unable to open or create cache db {:?}", p))?;
        // Create version table
        let _ = conn.execute(
            "CREATE TABLE IF NOT EXISTS sheet_version (name TEXT PRIMARY KEY, version TEXT)",
            [],
        )?;
        database_import::import_or_create_tables(excel, &mut conn, force_update)?;

        let map = get_define_dict(&mut conn)?;

        Ok(Database::new(conn, map))
    }

    pub fn get_name_in_table(
        &self,
        table: &str,
        name_column: &str,
        id: i32,
    ) -> Result<Option<String>> {
        let query_name_str = format!("SELECT {} FROM {} WHERE RowId = ?", name_column, table);

        let query_name: Option<String> =
            self.conn
                .query_row(&query_name_str, params![id], |row| row.get(0))?;

        match query_name {
            Some(val) if !val.is_empty() => Ok(Some(val)),
            _ => Ok(None),
        }
    }

    pub fn get_level_ref(&self, id: i32) -> Result<Option<String>> {
        let mut statement = self
            .conn
            .prepare_cached("SELECT Object,EventId,Type FROM Level WHERE RowId = ?")?;

        let mut query_name_2 = statement.query(params![id])?;

        if let Ok(Some(row)) = query_name_2.next() {
            let obj: i32 = row.get("Object")?;
            // let event: i32 = row.get("EventId")?;
            let obj_type: i32 = row.get("Type")?;
            match obj_type {
                5 => Ok(None), // Might be a position for script character creation
                6 => Ok(None), // ?
                8 => Ok(self
                    .get_name_in_table("ENpcResident", "Singular", obj)?
                    .map(|name| head_all(&name))),
                9 => Ok(self
                    .get_name_in_table("BNpcName", "Singular", obj)?
                    .map(|name| head_all(&name))),
                12 => Ok(None), // Aetheryte references?
                14 => Ok(None), // Gathering points
                49 if obj == 5000000 => Ok(Some("BASE_ID_EVENT_RANGE".to_string())),
                _ => Ok(None),
            }
        } else {
            Ok(None)
        }
    }

    pub fn get_item_qty_desc(
        &self,
        id: i32,
        count: i32,
        is_hq: bool,
        dye: i32,
    ) -> Result<Option<String>> {
        if id == 0 {
            return Ok(None);
        }
        let hq_suffix = if is_hq { " {{HQ}}" } else { "" };

        let dye_color = if dye > 0
            && let Some(x) = self.get_name_in_table("Stain", "Name", dye)?
        {
            format!(" ({x})")
        } else {
            String::new()
        };

        let table_name = match id {
            1..=65535 => "Item",
            2000000..3000000 => "EventItem",
            _ => {
                return Ok(None);
            }
        };

        let query_name_str = format!(
            "SELECT Name,Singular,Plural,StartsWithVowel FROM {} WHERE RowId = ?",
            table_name
        );

        let mut statement_item = self.conn.prepare_cached(&query_name_str)?;

        let (name, single, plural, vowel) = statement_item.query_row(params![id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i32>(3)? != 0,
            ))
        })?;

        // Up to 3: name, full name, qty
        let mut item_link_parts: Vec<String> = Vec::with_capacity(3);
        item_link_parts.push(name.clone());

        let temp_name;
        if count == 1 {
            // Noun ownership e.g. Azeyma's Sleeve
            if REGEX_OWNER_CHECK.is_match(&single) {
                temp_name = format!("{single}{dye_color}");
            } else if id < 65536 {
                // regular item, probably not significant
                let article = if vowel { "an" } else { "a" };
                temp_name = format!("{article} {single}{dye_color}");
            } else {
                // event item, probably significant
                temp_name = format!("the {single}{dye_color}");
            }
        } else {
            temp_name = format!("{plural}{dye_color}");
        }
        if temp_name != name {
            item_link_parts.push(temp_name);
        }
        if is_hq {
            item_link_parts.push("hq=true".to_string());
        }
        if count != 1 {
            item_link_parts.push(format!("qty={count}"));
        }

        Ok(Some(format!(
            "{{{{i|{}}}}}{hq_suffix}",
            item_link_parts.join("|")
        )))
    }

    pub fn get_item_qty(
        &self,
        id: i32,
        count: i32,
        is_hq: bool,
        dye: i32,
    ) -> Result<Option<String>> {
        if id == 0 {
            return Ok(None);
        }
        let item_name_col = if count == 1 { "Singular" } else { "Plural" };
        let hq = if is_hq { " HQ " } else { " " };
        let item_name = match id {
            1..65536 => self.get_name_in_table("Item", item_name_col, id)?,
            2000000..3000000 => self.get_name_in_table("EventItem", item_name_col, id)?,
            _ => None,
        }
        .unwrap_or(format!("#{id}"));

        let mut combined = format!("{count}{hq}{item_name}");
        if dye > 0
            && let Some(x) = self.get_name_in_table("Stain", "Name", dye)?
        {
            combined.push_str(format!(" ({})", head_all(&x)).as_str());
        }

        Ok(Some(combined))
    }

    pub fn get_event_npc(&self, id: i32) -> Result<Option<String>> {
        let name_opt = self.get_name_in_table("ENpcResident", "Singular", id)?;

        Ok(name_opt.map(|name| head_all(&name)))
    }

    pub fn get_name_from_id(&self, id: i32) -> Result<Option<String>> {
        match id {
            5000000 => Ok(Some("BASE_ID_EVENT_RANGE".to_string())),
            5010000 => Ok(Some("BASE_ID_PLAYER".to_string())),
            5020000 => Ok(Some("BASE_ID_TERRITORY_TYPE".to_string())),
            1000000..2000000 => Ok(self
                .get_name_in_table("ENpcResident", "Singular", id)?
                .map(|name| head_all(&name))),
            2000000..3000000 => Ok(self
                .get_name_in_table("EObjName", "Singular", id)?
                .map(|name| head_all(&name))),
            _ => Ok(None),
        }
    }

    pub fn get_all_quests(&self) -> Result<Vec<Quest>> {
        let mut statement_quest = self.conn.prepare("SELECT Quest.*,JournalGenre.Name GenreName,JournalCategory.Name CategoryName,JournalSection.Name SectionName FROM Quest left join JournalGenre on Quest.JournalGenre=JournalGenre.RowId left join JournalCategory on JournalGenre.JournalCategory=JournalCategory.RowId left join JournalSection on JournalCategory.JournalSection=JournalSection.RowId WHERE Quest.Id IS NOT NULL AND Quest.Id<>''")?;

        println!("Querying all quests");
        let mut query_quest = statement_quest.query(params![])?;

        let mut ret = Vec::new();
        println!("Query done. Parsing...");
        while let Some(row) = query_quest.next()? {
            ret.push(Quest::from_row(&self, row)?);
        }

        println!("Parsing done.");
        Ok(ret)
    }

    pub fn get_quest(&self, index: i32) -> Result<Option<Quest>> {
        let mut statement_quest = self.conn.prepare_cached("SELECT *,JournalGenre.Name GenreName,JournalCategory.Name CategoryName,JournalSection.Name SectionName FROM Quest left join JournalGenre on Quest.JournalGenre=JournalGenre.RowId left join JournalCategory on JournalGenre.JournalCategory=JournalCategory.RowId left join JournalSection on JournalCategory.JournalSection=JournalSection.RowId WHERE Quest.RowId = ?")?;

        let mut query_quest = statement_quest.query(params![index])?;

        match query_quest.next()? {
            Some(row) => Ok(Some(Quest::from_row(&self, row)?)),
            _ => Ok(None),
        }
    }

    pub fn get_quest_name(&self, index: i32) -> Result<Option<String>> {
        let query_quest = self
            .conn
            .prepare_cached("SELECT name FROM Quest WHERE RowId = ?")?
            .query_row(params![index], |row| row.get(0));

        match query_quest {
            Ok(name) => Ok(Some(name)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn get_quest_battle(&self, index: i32) -> Result<Option<QuestBattle>> {
        let mut statement_quest = self
            .conn
            .prepare_cached("SELECT * FROM QuestBattle WHERE RowId = ?")?;

        let mut query_quest = statement_quest.query(params![index])?;

        match query_quest.next()? {
            Some(row) => Ok(Some(QuestBattle::from_row(row)?)),
            _ => Ok(None),
        }
    }

    pub fn get_quest_text_map(
        &self,
        excel: &Excel,
        quest_id: &String,
    ) -> Result<HashMap<String, String>> {
        if quest_id.len() != 15 {
            return Err(anyhow!(
                "Couldn't fetch dialogue map: quest ID {quest_id} is not 15 characters long"
            ));
        }
        let folder = &quest_id[10..13];
        let path_dialogue = format!("quest/{}/{}", folder, quest_id);
        let quest_sheet = excel.sheet(path_dialogue)?;
        let cols = quest_sheet.columns()?;
        let mut ret = HashMap::new();
        for row in quest_sheet {
            let key = row.field(cols.get(0).unwrap())?;
            let val = row.field(cols.get(1).unwrap())?;
            if let (Field::String(k), Field::String(v)) = (key, val) {
                let k = format_string(excel, &k);
                let v = format_string(excel, &v);
                ret.insert(k, v);
            }
        }
        Ok(ret)
    }

    pub fn get_text_map(&self, excel: &Excel, path: &String) -> Result<HashMap<String, String>> {
        let quest_sheet = excel.sheet(path)?;
        let cols = quest_sheet.columns()?;
        let mut ret = HashMap::new();
        for row in quest_sheet {
            let key = row.field(cols.get(0).unwrap())?;
            let val = row.field(cols.get(1).unwrap())?;
            if let (Field::String(k), Field::String(v)) = (key, val) {
                let k = format_string(excel, &k);
                let v = format_string(excel, &v);
                ret.insert(k, v);
            }
        }
        Ok(ret)
    }

    pub fn get_matching_quests_by_title(&self, search: &String) -> Result<Vec<(i32, String, i32)>> {
        let mut res = Vec::new();

        let mut statement_exact = self
            .conn
            .prepare_cached("SELECT RowId,name,IssuerStart FROM Quest WHERE name = ?")?;
        let result_exact = statement_exact.query_map(params![search], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?;

        for row in result_exact {
            res.push(row?);
        }

        if res.len() == 1 {
            return Ok(res);
        }

        let mut statement_like = self
            .conn
            .prepare_cached("SELECT RowId,name,IssuerStart FROM Quest WHERE name LIKE ?")?;
        let search_like = format!("%{}%", search);
        let result_like = statement_like.query_map(params![search_like], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?;

        for row in result_like {
            res.push(row?);
        }

        Ok(res)
    }

    pub fn get_matching_quests_by_number(&self, search: i32) -> Result<Vec<(i32, String, i32)>> {
        let mut res = Vec::new();

        let mut statement = self
            .conn
            .prepare_cached("SELECT RowId,name,IssuerStart FROM Quest WHERE RowId = ?")?;
        let result = statement.query_map(params![search], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?;

        for row in result {
            res.push(row?);
        }

        Ok(res)
    }

    pub fn get_matching_quests_by_id(&self, search: &String) -> Result<Vec<(i32, String, i32)>> {
        let mut res = Vec::new();

        let mut statement = self
            .conn
            .prepare_cached("SELECT RowId,name,IssuerStart FROM Quest WHERE id LIKE ?")?;
        let search_like = format!("%{}%", search);
        let result = statement.query_map(params![search_like], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?;

        for row in result {
            res.push(row?);
        }

        Ok(res)
    }

    pub fn get_cutscene_path(&self, index: i32) -> Result<Option<String>> {
        let query_quest = self
            .conn
            .prepare_cached("SELECT Path FROM Cutscene WHERE RowId = ?")?
            .query_row(params![index], |row| row.get(0));

        match query_quest {
            Ok(name) => Ok(Some(name)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn get_bgm_link(&self, index: i32) -> Result<Option<String>> {
        let query_quest = self
            .conn
            .prepare_cached("SELECT File FROM BGM WHERE RowId = ?")?
            .query_row(params![index], |row| row.get(0));

        match query_quest {
            Ok(name) => Ok(Some(name)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
}

fn get_define_dict(conn: &mut Connection) -> Result<HashMap<String, i32>> {
    let mut res = HashMap::new();

    let mut statement =
        conn.prepare("SELECT Text,DefineValue FROM QuestSystemDefine WHERE Text<>''")?;
    let query_defines = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;

    for row in query_defines {
        let (key, val) = row?;
        res.insert(key, val);
    }

    statement = conn.prepare("SELECT Text,DefineValue FROM EventSystemDefine WHERE Text<>''")?;
    let query_event = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;

    for row in query_event {
        let (key, val) = row?;
        res.insert(key, val);
    }

    Ok(res)
}
