use anyhow::{Context, Result, anyhow};
use clap::{CommandFactory, Parser};
use regex::Regex;
use std::{
    collections::HashMap,
    fs::{self, File},
    io::{BufRead, BufReader},
    path::PathBuf,
    sync::Arc,
};

mod cgwiki;
mod database;
mod database_import;
mod exd_schema;
mod luadec;
mod model;
mod replace;
mod settings;
mod utility;
use ironworks::{
    Ironworks,
    excel::Excel,
    sqpack::{Install, SqPack},
};
mod detail_level;

use crate::{
    database::Database, detail_level::DetailLevel, luadec::chunk, model::quest::Quest,
    settings::Settings,
};

#[derive(Parser, Debug)]
#[command(about = "Extract quest dialogue.")]
struct Args {
    /// True to force a database update.
    #[arg(short = 'u', long)]
    force_update: bool,

    /// True to read all quests.
    #[arg(short = 'a', long)]
    read_all: bool,

    /// The detail level of the file.
    /// 1 = dialogue lines only (default)
    /// 2 = dialogue lines with some Lua script
    /// 3 = dialogue lines with all Lua script lines
    #[arg(short = 's', long)]
    detail_level: Option<i32>,

    /// The name of the file to write the dialogue to.
    #[arg(short, long)]
    output: Option<String>,

    /// The 5-digit quest number.
    #[arg(short = 'i', long)]
    quest_number: Option<i32>,

    /// All or part of the quest ID
    #[arg(short = 'q', long)]
    quest_id: Option<String>,

    /// All or part of the name of the quest.
    #[arg(short = 'n', long)]
    quest_name: Option<String>,

    /// The path to the FFXIV installation folder. Saves to config after program exits.
    #[arg(short = 'p', long)]
    ffxiv_install_path: Option<String>,

    /// A file containing a list of quest IDs or numbers, one per line.
    #[arg(short = 'f', long)]
    quest_id_file: Option<String>,
}

fn print_quest(
    database: &Database,
    ironworks: &Ironworks,
    excel: &Excel,
    quest: &Quest,
    args: &Args,
    func_report: &mut HashMap<i32, HashMap<String, i32>>,
) -> Result<String> {
    if quest.quest_id.len() != 15 {
        return Err(anyhow!(
            "Unable to fetch quest {}: quest ID missing",
            quest.number
        ));
    }
    let detail_level: DetailLevel = match args.detail_level {
        Some(i) => i.into(),
        None => DetailLevel::NoScript,
    };
    let mut output: String = String::new();
    let folder = &quest.quest_id[10..13];
    let script_path = format!("game_script/quest/{}/{}.luab", folder, quest.quest_id);
    let script_bytes: Vec<u8> = ironworks.file(script_path.as_str())?;
    let script_chunks = chunk::Chunk::parse(&script_bytes)?;
    let script = luadec::lib::decompile(&script_chunks);
    let dialogue_map = database
        .get_quest_text_map(&excel, &quest.quest_id)
        .with_context(|| anyhow!("Unable to find path for {}, skipping", quest.quest_id))?;
    if detail_level != DetailLevel::NoScript {
        output.push_str(format!("# {}: {}\n", quest.number, quest.name).as_str());
        output.push_str(format!("- Quest ID: {}\n", quest.quest_id).as_str());
        output.push_str(format!("- Genre: {}\n", quest.genre).as_str());
        output.push_str(format!("- Category: {}\n", quest.category).as_str());
        output.push_str(format!("- Section: {}\n", quest.section).as_str());
        output.push_str(format!("- Listener Start: {}\n", quest.listener_start).as_str());
        output.push_str(format!("- Listener End: {}\n", quest.listener_end).as_str());
        output.push_str(format!("- Required Level: {}\n", quest.required_level).as_str());
        if quest.required_job != "" {
            output.push_str(format!("- Required Job: {}\n", quest.required_job).as_str());
        }
        output.push_str(
            format!("- Required Quests: {}\n", quest.required_quests.join(", ")).as_str(),
        );
        if quest.quest_rewards.len() > 0 {
            output.push_str(format!("- Rewards: {}\n", quest.quest_rewards.join(", ")).as_str());
        }
        if quest.quest_option_rewards.len() > 0 {
            output.push_str(
                format!(
                    "- Reward Choice: {}\n",
                    quest.quest_option_rewards.join(", ")
                )
                .as_str(),
            );
        }
        if script.disassembled > 0 {
            output
                .push_str(format!("--Functions not decompiled: {}", script.disassembled).as_str());
        }
        for listener in &quest.quest_listeners {
            output.push_str(format!("- {listener}\n").as_str());
        }
        for todo in &quest.quest_todo {
            output.push_str(format!("{todo:?}\n").as_str());
        }
        output.push_str("\n");
    }

    let replace_result = replace::process(
        &excel,
        &ironworks,
        &database,
        &quest,
        &script.lines,
        detail_level,
        func_report,
    )?;

    output.push_str("==Dialogue==\n");

    for i in 0..24 {
        let tag = format!("TEXT_{}_TODO_{i:02}", quest.quest_id.to_uppercase());
        if let Some(rf) = dialogue_map
            .get(&tag)
            .and_then(|x| x.trim().strip_suffix("."))
        {
            if rf == "" {
                break;
            } else {
                output.push_str(format!("<!-- ==={}=== -->\n", rf).as_str());
            }
        }
    }

    for handover in replace_result.handover {
        output.push_str(format!("<!-- {} -->\n", handover).as_str());
    }

    output.push_str("{{Dialogue|<nowiki>\n===Accepting the quest===\n");
    let mut last_was_newline = false;
    for line in replace_result.script {
        let is_newline = line.trim().is_empty();

        if is_newline {
            if !last_was_newline {
                output.push_str(&format!("{}\n", line));
                last_was_newline = true;
            }
        } else {
            output.push_str(&format!("{}\n", line));
            last_was_newline = false;
        }
    }
    output.push_str("</nowiki>}}\n");
    let regex_replace_list: Vec<(Regex, &str)> = vec![
        (Regex::new(r"\t")?, "    "),
        (Regex::new(r".* = function(.*)\nend\n")?, ""),
        (Regex::new(r"\nend\n\n(\n)+")?, "\nend\n\n"),
        (Regex::new(r"> if.*\n---\n")?, ""),
        (Regex::new(r"\n\n+")?, "\n\n"),
        (Regex::new(r" +\n")?, "\n"),
    ];
    for (r, replace) in regex_replace_list {
        output = r.replace_all(&output, replace).into_owned();
    }
    Ok(output)
}

fn read_all(database: &Database, ironworks: &Ironworks, excel: &Excel, args: &Args) -> Result<()> {
    let output_path_mult = PathBuf::from("output");
    let mut func_report: HashMap<i32, HashMap<String, i32>> = HashMap::new();
    for quest in database.get_all_quests()? {
        if quest.quest_id.len() != 15 {
            println!("Skipping quest {} \"{}\"", quest.number, quest.quest_id);
            continue;
        }
        println!("Processing quest {}", quest.number);
        let output_path_specific =
            output_path_mult.join(format!("{} - {}.txt", quest.number, quest.quest_id));
        let output = print_quest(database, ironworks, excel, &quest, args, &mut func_report)?;
        if let Some(parent) = output_path_specific.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Unable to create out directory at {:?}", parent))?;
        }
        fs::write(output_path_specific, output)?;
    }
    Ok(())
}

fn read_multiple(
    database: &Database,
    ironworks: &Ironworks,
    excel: &Excel,
    args: &Args,
    id_list: &Vec<i32>,
) -> Result<()> {
    let output_path_mult = PathBuf::from("output");
    let mut func_report: HashMap<i32, HashMap<String, i32>> = HashMap::new();
    for i in id_list {
        let Some(quest) = database.get_quest(*i)? else {
            continue;
        };
        if quest.quest_id.len() != 15 {
            println!("Skipping quest {} \"{}\"", quest.number, quest.quest_id);
            continue;
        }
        println!("Processing quest {}", quest.number);
        let output_path_specific =
            output_path_mult.join(format!("{} - {}.txt", quest.number, quest.quest_id));
        let output = print_quest(database, ironworks, excel, &quest, args, &mut func_report)?;
        if let Some(parent) = output_path_specific.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Unable to create out directory at {:?}", parent))?;
        }
        fs::write(output_path_specific, output)?;
    }
    Ok(())
}

fn read_single(
    output_path_opt: Option<&String>,
    quest: &Quest,
    database: &Database,
    ironworks: &Ironworks,
    excel: &Excel,
    args: &Args,
) -> Result<()> {
    let mut func_report: HashMap<i32, HashMap<String, i32>> = HashMap::new();
    let output = print_quest(database, ironworks, excel, quest, args, &mut func_report)?;
    let output_path: PathBuf;
    if let Some(op) = output_path_opt {
        output_path = PathBuf::from(&op);
    } else {
        output_path = PathBuf::from(format!("{} - {}.txt", quest.number, &quest.quest_id[..9]));
    }
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Unable to create out directory at {:?}", parent))?;
    }
    fs::write(output_path, output)?;
    Ok(())
}

fn main() -> Result<()> {
    let args = Args::parse();

    let path_opt = if let Some(s) = &args.ffxiv_install_path {
        let p = PathBuf::from(s);
        if !p.exists() {
            return Err(anyhow!("Unable to find path '{s}'"));
        }
        Some(p)
    } else {
        None
    };

    settings::load(path_opt.as_ref())?;

    let settings = Settings::global();

    let ironworks: Arc<Ironworks> = Arc::new(
        Ironworks::new().with_resource(Box::new(SqPack::new(Install::at(&settings.game_path)))),
    );

    let mut excel = Excel::new(ironworks.clone());
    excel.set_default_language(ironworks::excel::Language::English);

    let database = Database::create(&excel, args.force_update)?;

    let mut quest_opt: Option<Quest> = None;
    let mut quest_list = Vec::new();
    let mut is_multiple = false;
    let mut id_list = Vec::new();

    if let Some(arg_list) = &args.quest_id_file {
        let p = PathBuf::from(arg_list);
        if !p.exists() {
            return Err(anyhow!(
                "Unable to find quest ID argument file '{arg_list}'"
            ));
        }
        let arg_file = File::open(p)?;
        let reader = BufReader::new(arg_file);
        for line in reader.lines() {
            let line = line?;
            if line.len() == 5
                && let Ok(num) = line.parse::<i32>()
            {
                let ql = database.get_matching_quests_by_number(num)?;
                if ql.len() == 0 {
                    println!("Warning: unable to find quests for number {num}")
                } else {
                    id_list.extend(ql.iter().map(|a| a.0));
                }
            } else if line.len() > 0 {
                let ql = database.get_matching_quests_by_id(&line)?;
                if ql.len() == 0 {
                    println!("Warning: unable to find quests for ID {line}")
                } else {
                    id_list.extend(ql.iter().map(|a| a.0));
                }
            }
        }
        is_multiple = true;
    } else if let Some(name) = &args.quest_name {
        quest_list.extend(database.get_matching_quests_by_title(&name)?);
    } else if let Some(num) = args.quest_number {
        quest_list.extend(database.get_matching_quests_by_number(num)?);
    } else if let Some(id) = &args.quest_id {
        quest_list.extend(database.get_matching_quests_by_id(&id)?);
    } else if !args.read_all {
        Args::command().print_help()?;
        return Ok(());
    }

    if !is_multiple && !args.read_all {
        if quest_list.len() == 0 {
            println!("No quests found with the provided ID.");
            return Ok(());
        } else if quest_list.len() > 1 {
            let mut opt_list = Vec::new();
            for (i, x) in quest_list.iter().take(10).enumerate() {
                let listener = database.get_event_npc(x.2)?.unwrap_or_default();
                if listener == "" {
                    opt_list.push(format!("{i}) {} ({})", x.0, x.1));
                } else {
                    opt_list.push(format!("{i}) {} ({}), issued by {}", x.0, x.1, listener));
                }
            }
            println!(
                "There are multiple quests that match. Enter the number of which quest to pick, or enter \"q\" or \"quit\" to quit."
            );
            for line in &opt_list {
                println!("{line}");
            }
            loop {
                let mut input = String::new();
                let _ = std::io::stdin().read_line(&mut input)?;
                input = input.trim().to_string();
                if input == "q" || input == "quit" {
                    break;
                } else if let Ok(index) = input.parse::<usize>()
                    && index < opt_list.len()
                {
                    quest_opt = database.get_quest(quest_list.get(0).unwrap().0)?;
                    break;
                }
            }
        } else {
            quest_opt = database.get_quest(quest_list.get(0).unwrap().0)?;
        }
    }

    if is_multiple {
        read_multiple(&database, &ironworks, &excel, &args, &id_list)?;
    } else if args.read_all {
        read_all(&database, &ironworks, &excel, &args)?;
    } else if let Some(quest) = quest_opt {
        read_single(
            args.output.as_ref(),
            &quest,
            &database,
            &ironworks,
            &excel,
            &args,
        )?;
    } else {
        println!("Unable to find quest")
    }
    let settings = Settings::global();
    settings::save(&settings)?;
    Ok(())
}
