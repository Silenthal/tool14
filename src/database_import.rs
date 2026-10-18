use anyhow::{Context, Result};
use ironworks::{
    excel::{Excel, Field},
    file::exh::ColumnKind,
};
use rusqlite::{Connection, params};

use crate::{
    cgwiki::{formatter::format_string, lookup::sheet_lookup::get_sheet_detail},
    settings::Settings,
};

const TABLE_LIST: &[&str] = &[
    // "ActionTimeline",
    // "Balloon",
    // "Behavior",
    "BGM",
    "BNpcBase",
    "BNpcName",
    "ClassJob",
    "Cutscene",
    "DefaultTalk",
    "ENpcBase",
    "ENpcResident",
    "EObj",
    "EObjName",
    "EventItem",
    "EventSystemDefine",
    "ExportedSG",
    "Item",
    "JournalCategory",
    "JournalGenre",
    "JournalSection",
    "Level",
    "LogMessage",
    "Map",
    "PlaceName",
    "Quest",
    "QuestBattle",
    // "QuestDefineClient",
    "QuestSetDefine",
    "QuestSystemDefine",
    "Stain",
];

pub fn import_or_create_tables(
    excel: &Excel,
    conn: &mut Connection,
    force_update: bool,
) -> Result<()> {
    let settings = Settings::global();
    for &table_name in TABLE_LIST {
        let cur_version = settings.get_game_ver()?;
        let is_table_up_to_date: bool = {
            let mut query_table_exists =
                conn.prepare("SELECT count(*) FROM sheet_version WHERE name = ? AND version = ?")?;
            let count: i32 =
                query_table_exists.query_row(params![table_name, cur_version], |row| row.get(0))?;
            count > 0
        };

        if !force_update && is_table_up_to_date {
            return Ok(());
        }

        println!("Updating table {table_name}");
        let mut new_table_create_column_list = vec![
            "\"RowId\" INTEGER".to_string(),
            "\"SubRowId\" INTEGER".to_string(),
        ];

        let mut new_table_insert_column_list =
            vec!["\"RowId\"".to_string(), "\"SubRowId\"".to_string()];
        let sheet_detail = get_sheet_detail(excel, table_name)
            .with_context(|| format!("Unable to retrieve column details for table {table_name}. Schema file possibly missing."))?;
        for (name, def) in &sheet_detail.col_def_list {
            let sql_type = match def.kind() {
                ColumnKind::String => "TEXT",
                ColumnKind::Float32 => "REAL",
                _ => "INTEGER",
            };
            new_table_create_column_list.push(format!("\"{}\" {}", name, sql_type));
            new_table_insert_column_list.push(format!("\"{}\"", name));
        }

        let sheet = excel.sheet(table_name)?;
        let mut new_table_insert_rows = Vec::new();
        for row in sheet.into_iter() {
            let mut row_data = vec![row.row_id().to_string(), row.subrow_id().to_string()];
            for (_, def) in &sheet_detail.col_def_list {
                let field = row.field(def)?;
                let field_val = match &field {
                    Field::String(value) => {
                        format!(
                            "\"{}\"",
                            &format_string(&excel, &value).replace("\"", "\"\"")
                        )
                    }
                    Field::Bool(value) => {
                        if *value {
                            "1".to_string()
                        } else {
                            "0".to_string()
                        }
                    }
                    Field::I8(value) => value.to_string(),
                    Field::I16(value) => value.to_string(),
                    Field::I32(value) => value.to_string(),
                    Field::I64(value) => value.to_string(),
                    Field::U8(value) => value.to_string(),
                    Field::U16(value) => value.to_string(),
                    Field::U32(value) => value.to_string(),
                    Field::U64(value) => value.to_string(),
                    Field::F32(value) => value.to_string(),
                };
                row_data.push(field_val);
            }
            new_table_insert_rows.push(row_data.join(","));
        }

        let new_table = format!("{table_name}_new");

        let transaction = conn.transaction()?;

        transaction.execute(format!("DROP TABLE IF EXISTS {new_table}").as_str(), [])?;

        transaction.execute(
            format!(
                "CREATE TABLE {new_table} ({}, PRIMARY KEY (RowId, SubRowId))",
                new_table_create_column_list.join(", ")
            )
            .as_str(),
            [],
        )?;

        let new_table_insert_columns = new_table_insert_column_list.join(",");
        for row in new_table_insert_rows {
            transaction.execute(
                format!(
                    "INSERT INTO {new_table} ({}) VALUES ({})",
                    new_table_insert_columns, row
                )
                .as_str(),
                [],
            )?;
        }

        transaction.execute(format!("DROP TABLE IF EXISTS {table_name}").as_str(), [])?;

        transaction.execute(
            format!("ALTER TABLE {new_table} RENAME TO {table_name}").as_str(),
            [],
        )?;

        transaction.execute(
            "INSERT OR REPLACE INTO sheet_version (name, version) VALUES (?, ?)",
            params![table_name, cur_version],
        )?;

        transaction.commit()?;
    }
    Ok(())
}
