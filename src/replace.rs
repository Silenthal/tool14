use std::{
    collections::{HashMap, HashSet},
    sync::LazyLock,
};

use crate::{
    database::Database,
    model::{cutscene_dialogue::CutsceneDialogue, item_detail::ItemDetail, quest::Quest},
};
use anyhow::Result;
use ironworks::{Ironworks, excel::Excel};
use levenshtein::levenshtein;
use regex::Regex;

#[derive(Default, Clone, Debug)]
struct LuaFunction {
    args: Vec<String>,
    // var_map: HashMap<String, LuaValue>,
}

#[derive(Default, Clone, Debug)]
enum LuaValue {
    #[default]
    Nil,
    // Bool(bool), // Not handling these currently
    Integer(i32),
    // Float(f64), // Not handling these currently
    String(String),
    Function(LuaFunction),
    Table(Vec<String>),
}

struct State<'a> {
    database: &'a Database,
    player_var: String,
    target_var: String,
    listener_key_to_name: HashMap<String, String>,
    var_to_name: HashMap<String, String>,
    self_var_map: HashMap<String, LuaValue>,
    local_var_map: HashMap<String, LuaValue>,
}

const NAME: &[(&str, &str)] = &[
    ("BEATINE", "Beatin"),
    ("FRANZ", "Franz the Fair"),
    ("LANGLOISIERT", "Langloisiert the Lionheart"),
    ("ULINBHO", "U'linbho the Sand Devil"),
    ("LETTERMOOGLE", "Deputy Postmoogle"),
    ("EDMONT", "Count Edmont de Fortemps"),
    ("LAURISSE", "Laurisse De Jervaint"),
    ("SILENTMOSS", "Silent Moss the Solemn"),
];

static NAME_LOOKUP: LazyLock<HashMap<String, String>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    for (x, y) in NAME {
        m.insert(x.to_string(), y.to_string());
    }
    m
});

const IGNORE: &[&str] = &[
    "AutoShake",
    "BattleMode",
    "BattleModeEx",
    "BeginCutScene",
    "CancelActionTimeline",
    "CancelActionTimelineAll",
    "CancelEventScene",
    "CancelMove",
    "CancelNpcTrade",
    "CancelSequentialEvent",
    "ChangeBGM",
    "ChangeBGMVolume",
    "ChangeEnvSoundVolume",
    "ChangeWeather",
    "CheckExclusiveQuestEffect",
    "CleanupLCutActors",
    "CloseGimmickMenu",
    "CloseHowTo",
    "CloseTalk",
    "ContinueEventBGM",
    "ContinueEventBGMUntilWarp",
    "Direction",
    "DisableSceneSkip",
    "DisableUIFunction",
    "Dismount",
    "EnableSceneSkip",
    "EnableUIFunction",
    "EndCutScene",
    "Equip",
    "EquipQuestModel",
    "EventPicture",
    "EventPictureOffset",
    "EyeLookAt",
    "EyeLookAtYawPitch",
    "FadeIn",
    "FadeOut",
    "FakeNotice",
    "FollowLookAt",
    "FootStep",
    "Gyro",
    "HowTo",
    "Idle",
    "InstanceDungeonOpenDisplay",
    "Inventory",
    "InvisibleStandCharacter",
    "InvisibleStandObject",
    "LastingBGM",
    "LearningAction",
    "LoadEventPicture",
    "LoadMovePosition",
    "LogMessageContentOpen",
    "LookAt",
    "LookAtCamera",
    "LookAtEID",
    "LookAtLayout",
    "LookAtTwoHead",
    "Move",
    "NoLookAtTurnToObject",
    "OpenDawnUI",
    "Orbit",
    "PathCurveWalkOut",
    "PathWalkOut",
    "PlayActionTimeline",
    "PlayBGM",
    "PlayBGMWithVolume",
    "PlayCamera",
    "PlayDirectionalAimingCamera",
    "PlayEmote",
    "PlayHandShake",
    "PlayLandscapeCamera",
    "PlayLandscopeCamera",
    "PlayManagedVfx",
    "PlayQuestGimmickReaction",
    "PlayScreenShake",
    "PlaySE",
    "PlaySharedGroupTimeline",
    "PlaySharedGroupTimelineSkippable",
    "PlaySharedGroupTimelineSkippable",
    "PlayStaffRoll",
    "PlayTargetAimingCamera",
    "PlayTargetRelationCamera",
    "PlayTwoShotCamera",
    "PlayVfx",
    "PlayWorldPositionCamera",
    "PointMenuChangeButtonState",
    "Position",
    "PrepareMovableEvent",
    "ProgressTodo",
    "QuestAccepted",
    "QuestEventTutorial",
    "ReserveSequentialEvent",
    "ResetBGM",
    "ResetSkip",
    "RevisibleStandObject",
    "SaveFestivalQuestWorkFlag",
    "ScreenGC",
    "ScreenImage",
    "SetFlying",
    "SetFreeWorkBitFlag",
    "SetFreeWorkU2",
    "SetFreeWorkU4",
    "SetLodHigh",
    "SetMount",
    "SetSceneEndRollback",
    "SetTemporaryValue",
    "SideDolly",
    "SidePan",
    "Skip",
    "StopAimingCamera",
    "StopEventBGM",
    "StopHandShake",
    "StopScreenShake",
    "SwimOut",
    "SyncWorkOperation",
    "Transparency",
    "TurnFromLayout",
    "TurnTo",
    "TurnToCamera",
    "TurnToLayout",
    "UpdownDolly",
    "UpdownPan",
    "Visible",
    "Wait",
    "WaitForActionTimeline",
    "WaitForDolly",
    "WaitForFade",
    "WaitForLoadEventPicture",
    "WaitForLookAt",
    "WaitForMove",
    "WaitForOrbit",
    "WaitForPan",
    "WaitForPathMove",
    "WaitForSharedGroupTimeline",
    "WaitForTransparency",
    "WaitForTurn",
    "WaitForWhiteFade",
    "WaitForZoom",
    "WalkIn",
    "WalkOut",
    "Weather",
    "WhiteFadeIn",
    "WhiteFadeOut",
    "WorldTime",
    "Zoom",
];

static IGNORE_LOOKUP: LazyLock<HashSet<String>> = LazyLock::new(|| {
    let mut s = HashSet::new();
    for x in IGNORE {
        s.insert(x.to_string());
    }
    s
});

const FUNCTION_SKIP: &[&str] = &[
    "IsTodoChecked",
    "GetTodoArgs",
    "GetGimmickState",
    "GetNpcTradeItems",
    "SCRIPT_VERSION",
    "IsAnnounce",
    "IsAcceptEvent",
    "PLANDEF_GetMenuTextLabels",
    "GetMenuTextLabels",
];

static FUNCTION_SKIP_LOOKUP: LazyLock<HashSet<String>> = LazyLock::new(|| {
    let mut s = HashSet::new();
    for x in FUNCTION_SKIP {
        s.insert(x.to_string());
    }
    s
});

const LABEL_FUNC: &[&str] = &["PLANDEF_GetMenuTextLabels", "GetMenuTextLabels"];

static LABEL_FUNC_LOOKUP: LazyLock<HashSet<String>> = LazyLock::new(|| {
    let mut s = HashSet::new();
    for x in LABEL_FUNC {
        s.insert(x.to_string());
    }
    s
});

pub fn process(
    excel: &Excel,
    ironworks: &Ironworks,
    database: &Database,
    quest: &Quest,
    dialogue_dict: &HashMap<String, String>,
    lines: &Vec<String>,
    is_all_script_instruction: bool,
    func_report: &mut HashMap<i32, HashMap<String, i32>>,
) -> Result<Vec<String>> {
    let quest_tag = &quest.quest_id[..9];
    let regex_top_level = Regex::new(format!(r"^{}.(\w+) = (.*)$", &quest_tag).as_str())?;
    let regex_assign = Regex::new(r"^\s+(?:local )?(.*) = (.*)$")?;
    let regex_self_var = Regex::new(r"self\.([\w+]+)")?;
    let regex_call = Regex::new(r"(\w+):(\w+)\(([^\)]*)\)")?;
    let regex_dialogue_alt_speaker = Regex::new(r"^\(-([^-]+)-\).*$")?;
    let regex_item_check = Regex::new(
        r"(\w+)\[(?<out_index>\d+)\] = \{\s*(?<item_name>[\w\.]+),\s*(?<item_count>\w+),\s*(?<item_is_hq>\w+),\s*(\w+),\s*(\w+),\s*(\w+),\s*(\w+),\s*(\w+),\s*(\w+),\s*(\w+),\s*(\w+),\s*(\w+),\s*(\w+),\s*(\w+)\s*\}",
    )?;
    let regex_start_item_block = Regex::new(r"([\w\.]+) == (?<listener_id>[\w\.]+) then$")?;
    let regex_unpack = Regex::new(r"unpack\((?<table_var>v\w+)\)")?;
    let regex_table_insert = Regex::new(r"table\.insert\(([\w]+), ([\w\.]+)(\[[\w\.]+\])?\)")?;
    let mut listener_to_item = HashMap::new();
    let mut item_listener_id = 0;
    let mut output: Vec<String> = Vec::new();
    let mut state: State = State {
        database,
        player_var: String::new(),
        target_var: String::new(),
        listener_key_to_name: HashMap::new(),
        var_to_name: HashMap::new(),
        self_var_map: HashMap::new(),
        local_var_map: HashMap::new(),
    };
    let mut target_name: Option<&String> = None;
    let mut clear_target_name = false;
    let mut skip_to_end = false;
    let mut trim_after = false;
    let mut script_version: i32 = 0;
    let mut inside_item_block = false;
    let mut inside_label_func = false;
    let mut inside_label_func_name = String::new();
    let mut temp_item_map = HashMap::new();
    let mut temp_text_list = Vec::new();

    if quest.listener_start != "" {
        state.listener_key_to_name.insert(
            name_to_name_key(&quest.listener_start),
            quest.listener_start.clone(),
        );
        target_name = Some(&quest.listener_start);
    }
    if quest.listener_end != "" {
        state.listener_key_to_name.insert(
            name_to_name_key(&quest.listener_end),
            quest.listener_end.clone(),
        );
    }
    for listener in &quest.quest_listeners {
        if listener.display != "" {
            state.listener_key_to_name.insert(
                name_to_name_key(&listener.display),
                listener.display.clone(),
            );
        }
    }

    for (k, v) in dialogue_dict {
        state
            .self_var_map
            .insert(k.clone(), LuaValue::String(v.clone()));
    }

    for (k, v) in &database.quest_sys_define_map {
        state.self_var_map.insert(k.clone(), LuaValue::Integer(*v));
    }

    for (k, v) in quest.quest_params.iter() {
        state.self_var_map.insert(k.clone(), LuaValue::Integer(*v));
    }

    for line in lines {
        if inside_label_func && let Some(cap) = regex_assign.captures(line) {
            let (_, [var, val]) = cap.extract();
            temp_text_list.push(val.to_string());
        } else if inside_label_func && line == "end" {
            inside_label_func = false;
            state.self_var_map.insert(
                inside_label_func_name,
                LuaValue::Table(temp_text_list.clone()),
            );
            temp_text_list.clear();
            inside_label_func_name = String::new();
        } else if let Some(cap) = regex_top_level.captures(line) {
            let (_, [var, val]) = cap.extract();
            if LABEL_FUNC_LOOKUP.contains(var) {
                inside_label_func = true;
                inside_label_func_name = var.to_string();
                continue;
            } else if let Some(arg) = get_lua_val(val) {
                if var == "SCRIPT_VERSION"
                    && let LuaValue::Integer(s) = &arg
                {
                    script_version = *s;
                    func_report
                        .entry(script_version)
                        .or_insert_with(|| HashMap::new());
                }
                state.self_var_map.insert(var.to_string(), arg);
            }
        } else if let Some(cap) = regex_start_item_block.captures(line) {
            let listener_id_s = &cap["listener_id"];
            item_listener_id = if let Ok(i) = listener_id_s.parse::<i32>() {
                i
            } else if let Some(LuaValue::Integer(it)) =
                get_self_var_value(&state, &Some(listener_id_s.to_string()))
            {
                it
            } else {
                continue;
            };

            inside_item_block = true;
        } else if let Some(cap) = regex_item_check.captures(line) {
            let item_name_s = &cap["item_name"];
            let count_s = &cap["item_count"];
            let is_hq_s = &cap["item_is_hq"];
            let item_name_index = if let Ok(i) = item_name_s.parse::<i32>() {
                i
            } else if let Some(LuaValue::Integer(it)) =
                get_self_var_value(&state, &Some(item_name_s.to_string()))
            {
                it
            } else {
                continue;
            };
            let Ok(count_i) = count_s.parse::<i32>() else {
                continue;
            };
            let is_hq = is_hq_s == "true";
            let idtl = ItemDetail::new(item_name_index, count_i, is_hq);
            temp_item_map
                .entry((idtl.item_id, idtl.is_hq))
                .and_modify(|entry: &mut ItemDetail| entry.count += idtl.count)
                .or_insert(idtl);
        } else if inside_item_block && line.trim() == "end" {
            inside_item_block = false;
            if temp_item_map.len() > 0 {
                let mut temp_item_str_list = Vec::new();
                for (_, v) in &temp_item_map {
                    let Ok(Some(item_desc)) =
                        database.get_item_qty_desc(v.item_id, v.count, v.is_hq, 0)
                    else {
                        continue;
                    };
                    temp_item_str_list.push(item_desc);
                }
                let combined = match temp_item_str_list.len() {
                    1 => temp_item_str_list[0].to_string(),
                    2 => format!("{} and {}", temp_item_str_list[0], temp_item_str_list[1]),
                    _ => {
                        let first_part =
                            temp_item_str_list[..temp_item_str_list.len() - 1].join(", ");
                        format!(
                            "{}, and {}",
                            first_part,
                            temp_item_str_list[temp_item_str_list.len() - 1]
                        )
                    }
                };
                let statement = format!("''The player hands over {combined}.''");
                // output.push(statement.clone());
                listener_to_item
                    .entry(item_listener_id)
                    .and_modify(|list: &mut Vec<String>| list.push(statement.clone()))
                    .or_insert(vec![statement.clone()]);
                temp_item_map.clear();
                item_listener_id = 0;
            }
        }
    }

    for (k, v) in &listener_to_item {
        let listener = match database.get_event_npc(*k) {
            Ok(Some(s)) => s,
            _ => k.to_string(),
        };
        for item_line in v {
            output.push(format!("{listener}: {item_line}"));
        }
    }

    for line in lines {
        if line.starts_with("--") || line.starts_with("print(") {
            continue;
        }
        if line.contains("self:BeginCutScene()") {
            // Skip heuristic
            target_name = None;
        }
        if line == "end" {
            if skip_to_end {
                skip_to_end = false;
            } else {
                output.push(format!("{}", line.clone()));
                output.push(String::new());
            }
            continue;
        } else if skip_to_end {
            continue;
        }

        if line.starts_with(quest_tag) {
            state.local_var_map.clear();
            if clear_target_name {
                target_name = None;
                clear_target_name = false;
            }
            if let Some(cap) = regex_top_level.captures(line) {
                let (_, [var, _]) = cap.extract();
                if !is_all_script_instruction && FUNCTION_SKIP_LOOKUP.contains(var) {
                    skip_to_end = true;
                    continue;
                }
                if let Some(val) = state.self_var_map.get(var)
                    && let LuaValue::Function(func) = val
                {
                    if let Some(player_var) = func.args.get(1) {
                        state.player_var = player_var.clone();
                    }
                    if let Some(target_var) = func.args.get(2) {
                        state.target_var = target_var.clone();
                    }
                }
            }
            output.push(line.clone());
            continue;
        }

        if let Some(cap) = regex_table_insert.captures(line) {
            let table_var = &cap[1];
            let assigned = &cap[2];
            let list_opt =
                if let Some(LuaValue::Table(x)) = state.local_var_map.get(&table_var.to_string()) {
                    Some(x.clone())
                } else {
                    None
                };

            if let Some(mut list) = list_opt {
                if assigned.starts_with("self.") && !list.contains(&assigned.to_string()) {
                    list.push(assigned.to_string());
                }
                if let Some(lv) = state.local_var_map.get(assigned) {
                    match lv {
                        LuaValue::Table(other) => {
                            for val in other {
                                if !list.contains(val) {
                                    list.push(val.clone());
                                }
                            }
                        }
                        LuaValue::String(s) => {
                            list.push(s.clone());
                        }
                        _ => {}
                    }
                }
                state
                    .local_var_map
                    .insert(table_var.to_string(), LuaValue::Table(list));
            } else {
                break;
            }
        }
        let mut temp_line = line.clone();
        let mut temp_transform = Vec::new();
        let mut replaces = Vec::new();
        temp_line = regex_unpack
            .replace_all(&temp_line, |cap: &regex::Captures| {
                let var_name = &cap["table_var"];
                // if let Some(LuaValue::Table(x)) = state.local_var_map.get(var_name) {
                //     if x.len() == 0 {
                //         "{}".to_string()
                //     } else {
                //         format!("{}", x.join(", ").trim())
                //     }
                // } else {
                //     cap[0].to_string()
                // }
                let pre = cap[0].to_string();
                let rep = format!("unpack!{}", var_name);
                temp_transform.push((pre, rep));
                format!("unpack!{}", var_name)
            })
            .to_string();
        if let Some(cap) = regex_assign.captures(&temp_line) {
            let (_, [vars, vals]) = cap.extract();

            let variable_list: Vec<String> =
                vars.split("λ ").map(|x| x.trim().to_string()).collect();
            let mut value_list: Vec<String> =
                vals.split("λ ").map(|x| x.trim().to_string()).collect();
            let val_check = if value_list.len() == 1 {
                let pre = value_list[0]
                    .strip_prefix("self:")
                    .and_then(|s| Some(s.to_string()))
                    .unwrap_or(value_list[0].to_string());
                let post = pre
                    .strip_suffix("()")
                    .and_then(|s| Some(s.to_string()))
                    .unwrap_or(pre.clone());
                post.to_string()
            } else {
                String::new()
            };
            if value_list.len() == 1 && LABEL_FUNC_LOOKUP.contains(&val_check) {
                let Some(ff) = state.self_var_map.get(&val_check) else {
                    break;
                };
                let LuaValue::Table(table) = ff else {
                    break;
                };
                if table.len() > 0 {
                    value_list.clear();
                    for val in table {
                        value_list.push(val.clone());
                    }
                }
            }
            if variable_list.len() <= value_list.len() {
                for index in 0..variable_list.len() {
                    let var = variable_list.get(index).unwrap().clone();
                    let val = value_list.get(index).unwrap().clone();
                    if let Some(lv) = eval_func(database, &state, &val)? {
                        state.local_var_map.insert(var, lv);
                    }
                }
            }
        }

        let replace_res = regex_call.replace_all(&temp_line, |cap: &regex::Captures| {
            let (_, [var_caller, func, args]) = cap.extract();
            if !state.self_var_map.contains_key(func){
                func_report
                .entry(script_version)
                .and_modify(|e|{
                    let _ = e
                    .entry(func.to_string())
                    .and_modify(|i|*i += 1)
                    .or_insert(1);
                });
            }
            let temp_arg_list = args.split(", ").map(|x| x.trim().to_string());
            let mut arg_list_ls = Vec::new();
            let mut ups = false;
            for arg in temp_arg_list{
                // TODO: Here
                if arg.starts_with("unpack!"){
                    let strip_arg = arg.strip_prefix("unpack!").unwrap();
                    if let Some(LuaValue::Table(x)) = state.local_var_map.get(strip_arg) {
                        if x.len() == 0 {
                            arg_list_ls.push(format!("unpack({})", strip_arg));
                        } else {
                            for a in x{
                                arg_list_ls.push(a.clone());
                                ups = true;
                            }
                        }
                    } else {
                        arg_list_ls.push(arg);
                    }
                } else {
                    arg_list_ls.push(arg);
                }
            }
            match func {
                "Talk" => {
                    let mut arg_list = arg_list_ls.into_iter();
                    let _ = arg_list.next(); // player
                    let _ = arg_list.next(); // target
                    let dialogue_key_opt = get_self_var_name(&arg_list.next()); // actual line
                    let _ = arg_list.next(); // true to close dialog box
                    let _ = arg_list.next(); // design
                    let _ = arg_list.next(); // action
                    let _ = arg_list.next(); // lip 1
                    let _ = arg_list.next(); // lip 2?
                    // let local_dialogue_args: Vec<String> = arg_list.collect();
                    if let Some(speaker_key) = get_id(&dialogue_key_opt) {
                        let mut final_speaker = speaker_key.clone();
                        let mut alt_name_speaker = String::new();
                        if let Some(LuaValue::String(mut final_dialogue_line)) =
                            get_self_var_value(&state, &dialogue_key_opt)
                        {
                            // If this is the *first* talk, it could be from the quest assigner
                            if var_caller == state.target_var
                                && let Some(name_ref) = target_name
                            {
                                final_speaker = name_ref.clone();
                                clear_target_name = true;
                            }
                            // If var was assigned as the listener before, use that name
                            else if let Some(speaker) = state.var_to_name.get(var_caller) {
                                final_speaker = speaker.clone();
                            }
                            // If var was just assigned a string by bind/create char at some point, use that name
                            else if let Some(LuaValue::String(x)) =
                                state.local_var_map.get(var_caller)
                            {
                                state.var_to_name.insert(var_caller.to_string(), x.clone());
                                final_speaker = x.clone();
                            }
                            // If var is somewhere in the listener name list, use that name
                            else if let Some(speaker) =
                                find_closest_name(&mut state, &speaker_key, 2)
                            {
                                if var_caller != state.target_var {
                                    state
                                        .var_to_name
                                        .insert(var_caller.to_string(), speaker.clone());
                                }
                                final_speaker = speaker;
                            } else if let Some(speaker) = NAME_LOOKUP.get(&speaker_key) {
                                final_speaker = speaker.clone();
                            }
                            if let Some(caps) =
                                regex_dialogue_alt_speaker.captures(&final_dialogue_line)
                            {
                                let (_, [alt_name]) = caps.extract();
                                alt_name_speaker = format!("@{alt_name}");
                                final_dialogue_line = final_dialogue_line
                                    .strip_prefix(format!("(-{alt_name}-)").as_str())
                                    .unwrap_or(&final_dialogue_line)
                                    .to_string();
                            }
                            trim_after = !is_all_script_instruction;
                            let prefix = if is_all_script_instruction {
                                format!("{}\n", cap[0].to_string())
                            } else {
                                String::new()
                            };
                            format!(
                                "{prefix}\n{final_speaker}{alt_name_speaker}: {final_dialogue_line}\n"
                            )
                        } else {
                            cap[0].to_string()
                        }
                    } else {
                        cap[0].to_string()
                    }
                }
                "TalkAsync" => {
                    let mut arg_list = arg_list_ls.into_iter();
                    let _ = arg_list.next(); // player
                    let _ = arg_list.next(); // target
                    let dialogue_key_opt = get_self_var_name(&arg_list.next()); // actual line
                    let _ = arg_list.next(); // design
                    let _ = arg_list.next(); // action
                    let _ = arg_list.next(); // lip 1
                    let _ = arg_list.next(); // lip 2?
                    // let local_dialogue_args: Vec<String> = arg_list.collect();
                    if let Some(speaker_key) = get_id(&dialogue_key_opt) {
                        let mut final_speaker = speaker_key.clone();
                        let mut alt_name_speaker = String::new();
                        if let Some(LuaValue::String(mut final_dialogue_line)) =
                            get_self_var_value(&state, &dialogue_key_opt)
                        {
                            // If this is the *first* talk, it could be from the quest assigner
                            if var_caller == state.target_var
                                && let Some(name_ref) = target_name
                            {
                                final_speaker = name_ref.clone();
                                clear_target_name = true;
                            }
                            // If var was assigned as the listener before, use that name
                            else if let Some(speaker) = state.var_to_name.get(var_caller) {
                                final_speaker = speaker.clone();
                            }
                            // If var was just assigned a string by bind/create char at some point, use that name
                            else if let Some(LuaValue::String(x)) =
                                state.local_var_map.get(var_caller)
                            {
                                state.var_to_name.insert(var_caller.to_string(), x.clone());
                                final_speaker = x.clone();
                            }
                            // If var is somewhere in the listener name list, use that name
                            else if let Some(speaker) =
                                find_closest_name(&mut state, &speaker_key, 2)
                            {
                                if var_caller != state.target_var {
                                    state
                                        .var_to_name
                                        .insert(var_caller.to_string(), speaker.clone());
                                }
                                final_speaker = speaker;
                            } else if let Some(speaker) = NAME_LOOKUP.get(&speaker_key) {
                                final_speaker = speaker.clone();
                            }
                            if let Some(caps) =
                                regex_dialogue_alt_speaker.captures(&final_dialogue_line)
                            {
                                let (_, [alt_name]) = caps.extract();
                                alt_name_speaker = format!("@{alt_name}");
                                final_dialogue_line = final_dialogue_line
                                    .strip_prefix(format!("(-{alt_name}-)").as_str())
                                    .unwrap_or(&final_dialogue_line)
                                    .to_string();
                            }
                            trim_after = !is_all_script_instruction;
                            let prefix = if is_all_script_instruction {
                                format!("{}\n", cap[0].to_string())
                            } else {
                                String::new()
                            };
                            format!(
                                "{prefix}\n{final_speaker}{alt_name_speaker}: {final_dialogue_line}\n"
                            )
                        } else {
                            cap[0].to_string()
                        }
                    } else {
                        cap[0].to_string()
                    }
                }
                "ShortTalk" => {
                    let mut arg_list = arg_list_ls.into_iter();
                    let _ = arg_list.next(); // target
                    let dialogue_key_opt = get_self_var_name(&arg_list.next()); // actual line
                    let _ = arg_list.next(); // design
                    let _ = arg_list.next(); // action
                    let _ = arg_list.next(); // time
                    // let local_dialogue_args: Vec<String> = arg_list.collect();
                    if let Some(speaker_key) = get_id(&dialogue_key_opt) {
                        let mut final_speaker = speaker_key.clone();
                        let mut alt_name_speaker = String::new();
                        if let Some(LuaValue::String(mut final_dialogue_line)) =
                            get_self_var_value(&state, &dialogue_key_opt)
                        {
                            // If this is the *first* talk, it could be from the quest assigner
                            if var_caller == state.target_var
                                && let Some(name_ref) = target_name
                            {
                                final_speaker = name_ref.clone();
                                clear_target_name = true;
                            }
                            // If var was assigned as the listener before, use that name
                            else if let Some(speaker) = state.var_to_name.get(var_caller) {
                                final_speaker = speaker.clone();
                            }
                            // If var was just assigned a string by bind/create char at some point, use that name
                            else if let Some(LuaValue::String(x)) =
                                state.local_var_map.get(var_caller)
                            {
                                state.var_to_name.insert(var_caller.to_string(), x.clone());
                                final_speaker = x.clone();
                            }
                            // If var is somewhere in the listener name list, use that name
                            else if let Some(speaker) =
                                find_closest_name(&mut state, &speaker_key, 2)
                            {
                                if var_caller != state.target_var {
                                    state
                                        .var_to_name
                                        .insert(var_caller.to_string(), speaker.clone());
                                }
                                final_speaker = speaker;
                            } else if let Some(speaker) = NAME_LOOKUP.get(&speaker_key) {
                                final_speaker = speaker.clone();
                            }
                            if let Some(caps) =
                                regex_dialogue_alt_speaker.captures(&final_dialogue_line)
                            {
                                let (_, [alt_name]) = caps.extract();
                                alt_name_speaker = format!("@{alt_name}");
                                final_dialogue_line = final_dialogue_line
                                    .strip_prefix(format!("(-{alt_name}-)").as_str())
                                    .unwrap_or(&final_dialogue_line)
                                    .to_string();
                            }
                            trim_after = !is_all_script_instruction;
                            let prefix = if is_all_script_instruction {
                                format!("{}\n", cap[0].to_string())
                            } else {
                                String::new()
                            };
                            format!(
                                "{prefix}\n{final_speaker}{alt_name_speaker}: {final_dialogue_line}\n"
                            )
                        } else {
                            cap[0].to_string()
                        }
                    } else {
                        cap[0].to_string()
                    }
                }
                "ShortTalkWithLineVoice" => {
                    let mut arg_list = arg_list_ls.into_iter();
                    let _ = arg_list.next(); // target
                    let dialogue_key_opt = get_self_var_name(&arg_list.next()); // actual line
                    let _ = arg_list.next(); // design
                    let _ = arg_list.next();
                    let _ = arg_list.next();
                    let _ = arg_list.next(); // time
                    // let local_dialogue_args: Vec<String> = arg_list.collect();
                    if let Some(speaker_key) = get_id(&dialogue_key_opt) {
                        let mut final_speaker = speaker_key.clone();
                        let mut alt_name_speaker = String::new();
                        if let Some(LuaValue::String(mut final_dialogue_line)) =
                            get_self_var_value(&state, &dialogue_key_opt)
                        {
                            // If this is the *first* talk, it could be from the quest assigner
                            if var_caller == state.target_var
                                && let Some(name_ref) = target_name
                            {
                                final_speaker = name_ref.clone();
                                clear_target_name = true;
                            }
                            // If var was assigned as the listener before, use that name
                            else if let Some(speaker) = state.var_to_name.get(var_caller) {
                                final_speaker = speaker.clone();
                            }
                            // If var was just assigned a string by bind/create char at some point, use that name
                            else if let Some(LuaValue::String(x)) =
                                state.local_var_map.get(var_caller)
                            {
                                state.var_to_name.insert(var_caller.to_string(), x.clone());
                                final_speaker = x.clone();
                            }
                            // If var is somewhere in the listener name list, use that name
                            else if let Some(speaker) =
                                find_closest_name(&mut state, &speaker_key, 2)
                            {
                                if var_caller != state.target_var {
                                    state
                                        .var_to_name
                                        .insert(var_caller.to_string(), speaker.clone());
                                }
                                final_speaker = speaker;
                            } else if let Some(speaker) = NAME_LOOKUP.get(&speaker_key) {
                                final_speaker = speaker.clone();
                            }
                            if let Some(caps) =
                                regex_dialogue_alt_speaker.captures(&final_dialogue_line)
                            {
                                let (_, [alt_name]) = caps.extract();
                                alt_name_speaker = format!("@{alt_name}");
                                final_dialogue_line = final_dialogue_line
                                    .strip_prefix(format!("(-{alt_name}-)").as_str())
                                    .unwrap_or(&final_dialogue_line)
                                    .to_string();
                            }
                            trim_after = !is_all_script_instruction;
                            let prefix = if is_all_script_instruction {
                                format!("{}\n", cap[0].to_string())
                            } else {
                                String::new()
                            };
                            format!(
                                "{prefix}\n{final_speaker}{alt_name_speaker}: {final_dialogue_line}\n"
                            )
                        } else {
                            cap[0].to_string()
                        }
                    } else {
                        cap[0].to_string()
                    }
                }
                "BalloonTalk" => {
                    let mut arg_list = arg_list_ls.into_iter();
                    let _ = arg_list.next(); // player
                    let dialogue_key_opt = get_self_var_name(&arg_list.next()); // actual line
                    let _ = arg_list.next(); // time
                    // let local_dialogue_args: Vec<String> = arg_list.collect();
                    if let Some(speaker_key) = get_id(&dialogue_key_opt) {
                        let mut final_speaker = speaker_key.clone();
                        let mut alt_name_speaker = String::new();
                        if let Some(LuaValue::String(mut final_dialogue_line)) =
                            get_self_var_value(&state, &dialogue_key_opt)
                        {
                            // If this is the *first* talk, it could be from the quest assigner
                            if var_caller == state.target_var
                                && let Some(name_ref) = target_name
                            {
                                final_speaker = name_ref.clone();
                                clear_target_name = true;
                            }
                            // If var was assigned as the listener before, use that name
                            else if let Some(speaker) = state.var_to_name.get(var_caller) {
                                final_speaker = speaker.clone();
                            }
                            // If var was just assigned a string by bind/create char at some point, use that name
                            else if let Some(LuaValue::String(x)) =
                                state.local_var_map.get(var_caller)
                            {
                                state.var_to_name.insert(var_caller.to_string(), x.clone());
                                final_speaker = x.clone();
                            }
                            // If var is somewhere in the listener name list, use that name
                            else if let Some(speaker) =
                                find_closest_name(&mut state, &speaker_key, 2)
                            {
                                if var_caller != state.target_var {
                                    state
                                        .var_to_name
                                        .insert(var_caller.to_string(), speaker.clone());
                                }
                                final_speaker = speaker;
                            } else if let Some(speaker) = NAME_LOOKUP.get(&speaker_key) {
                                final_speaker = speaker.clone();
                            }
                            if let Some(caps) =
                                regex_dialogue_alt_speaker.captures(&final_dialogue_line)
                            {
                                let (_, [alt_name]) = caps.extract();
                                alt_name_speaker = format!("@{alt_name}");
                                final_dialogue_line = final_dialogue_line
                                    .strip_prefix(format!("(-{alt_name}-)").as_str())
                                    .unwrap_or(&final_dialogue_line)
                                    .to_string();
                            }
                            trim_after = !is_all_script_instruction;
                            let prefix = if is_all_script_instruction {
                                format!("{}\n", cap[0].to_string())
                            } else {
                                String::new()
                            };
                            format!(
                                "{prefix}\n{final_speaker}{alt_name_speaker}: {final_dialogue_line}\n"
                            )
                        } else {
                            cap[0].to_string()
                        }
                    } else {
                        cap[0].to_string()
                    }
                }
                "SystemTalk" => {
                    let mut arg_list = arg_list_ls.into_iter();
                    let dialogue_key_opt = get_self_var_name(&arg_list.next()); // actual line
                    let _ = arg_list.next(); // true to close dialog box
                    // let local_dialogue_args: Vec<String> = arg_list.collect();
                    if let Some(_) = get_id(&dialogue_key_opt) {
                        let Some(LuaValue::String(final_dialogue_line)) =
                            get_self_var_value(&state, &dialogue_key_opt)
                        else {
                            return cap[0].to_string();
                        };
                        trim_after = !is_all_script_instruction;
                        let prefix = if is_all_script_instruction {
                            format!("{}\n", cap[0].to_string())
                        } else {
                            String::new()
                        };
                        format!("{prefix}\nSystem: {final_dialogue_line}\n")
                    } else {
                        cap[0].to_string()
                    }
                }
                "Menu" | "GimmickMenu" | "ResultMenu" => {
                    let mut arg_list = arg_list_ls.into_iter();
                    let question_opt = get_self_var_name(&arg_list.next()); // actual line
                    let final_dialogue_line: String;
                    let Ok(Some(LuaValue::String(question))) =
                        eval_func(&database, &state, &question_opt.unwrap())
                    else {
                        return cap[0].to_string();
                    };
                    let mut line_list = vec![format!("Question: {question}")];
                    for answers in arg_list {
                        if let Some(answer_key) = get_self_var_name(&Some(answers)) {
                            let lookup = get_self_var_value(&state, &Some(answer_key.clone()));
                            if let Some(LuaValue::String(answer_s)) =
                                lookup
                            {
                                line_list.push(format!("> {answer_s}"));
                            }
                        }
                    }
                    final_dialogue_line = line_list.join("\n");
                    format!("{}\n\n{final_dialogue_line}\n", cap[0].to_string())
                }
                "YesNo" => {
                    let mut arg_list = arg_list_ls.into_iter();
                    let question_opt = get_self_var_name(&arg_list.next()); // actual line
                    let final_dialogue_line: String;
                    let Some(LuaValue::String(question)) =
                        get_self_var_value(&state, &question_opt)
                    else {
                        return cap[0].to_string();
                    };
                    final_dialogue_line = format!("Question: {question}\n> Yes\n> No");
                    format!("{}\n\n{final_dialogue_line}\n", cap[0].to_string())
                }
                "YesNoQuestBattle" => {
                    // Note: only works for below 200 - tables InstanceContent and InstanceContentTextData have text with no NPCs attached
                    let mut arg_list = arg_list_ls.into_iter();
                    let question_opt = get_self_var_value(&state, &arg_list.next());
                    if let Some(LuaValue::Integer(i)) = &question_opt && let Some(qb) = database.get_quest_battle(*i).ok().flatten() {
                        let mut temp = vec![cap[0].to_string()];
                        let mut paths = Vec::new();
                        for (_, v) in qb.quest_battle_params.iter().filter(|(k, _)|k.starts_with("CUT_SCENE")){
                            let Some(cut_path_opt) = database.get_cutscene_path(*v).ok().flatten() else{
                                continue;
                            };
                            paths.push(cut_path_opt);
                        }
                        paths.sort_by_key(|path| {
                            let filename = path.split('/').last().unwrap_or("");
                            let name_part = filename.rsplit('.').next().unwrap_or("");
                            let start_index = name_part.len() - 5;
                            name_part[start_index..].parse::<u32>().unwrap_or(0)
                        });
                        for cut_path in paths {
                            let Ok(mut cutscene_text) = get_cutscene_text_by_path(ironworks, excel, database, &cut_path, is_all_script_instruction) else{
                                continue;
                            };
                            temp.append(&mut cutscene_text);
                            temp.push(String::new());
                        }
                        temp.join("\n")
                    }
                    else{
                        cap[0].to_string()
                    }
                }
                "PlayCutScene" => {
                    let mut arg_list = arg_list_ls.into_iter();
                    let log_key_opt = get_self_var_value(&state, &arg_list.next());
                    let Some(LuaValue::Integer(cut_index)) = log_key_opt else{
                        return cap[0].to_string();
                    };
                    let Ok(dialogue_list) = get_cutscene_text_by_index(ironworks, excel, database, cut_index, is_all_script_instruction) else {
                        return cap[0].to_string();
                    };

                    let mut dialogue_str = String::new();
                    trim_after = !is_all_script_instruction;
                    if dialogue_list.len() > 0{
                        dialogue_str = format!("\n{}\n", dialogue_list.join("\n"));
                    }
                    let prefix = if is_all_script_instruction {
                        format!("{}\n", cap[0].to_string())
                    } else {
                        String::new()
                    };
                    format!("{}{}", prefix, dialogue_str)
                }
                "LogMessage" | "ScenarioMessage" => {
                    let mut arg_list = arg_list_ls.into_iter();
                    let log_key_opt = get_self_var_value(&state, &arg_list.next());
                    // let local_dialogue_args: Vec<String> = arg_list.collect();
                    let line_val = match log_key_opt{
                        Some(LuaValue::Integer(log_key)) => {
                            database
                                .get_name_in_table("LogMessage", "Text", log_key)
                                .ok()
                                .flatten()
                        },
                        Some(LuaValue::String(s)) => {
                            Some(s)
                        }
                        _ => None
                    };
                    let Some(final_dialogue_line) = line_val else{
                        return cap[0].to_string();
                    };
                    trim_after = !is_all_script_instruction;
                    let prefix = if is_all_script_instruction {
                        format!("{}\n", cap[0].to_string())
                    } else {
                        String::new()
                    };
                    format!("{prefix}\nPopup: {final_dialogue_line}\n")
                }
                "IsQuestAccepted" | "IsQuestCompleted" | "GetQuestSequence" => {
                    let mut arg_list = arg_list_ls.into_iter();
                    let quest_link_opt = arg_list.next();
                    if let Some(LuaValue::Integer(index)) =
                        get_self_var_value(&state, &quest_link_opt)
                        && let Ok(Some(quest_name)) = database.get_quest_name(index)
                    {
                        format!("{var_caller}:{func}({})", quest_name)
                    } else {
                        cap[0].to_string()
                    }
                }
                "PlayBGM" => {
                    let mut arg_list = arg_list_ls.into_iter();
                    let bgm_link_opt = arg_list.next();
                    trim_after = !is_all_script_instruction;
                    let prefix = if is_all_script_instruction {
                        format!("{}\n", cap[0].to_string())
                    } else {
                        String::new()
                    };
                    let bgm_name_final;
                    if let Some(LuaValue::Integer(index)) =
                        get_self_var_value(&state, &bgm_link_opt)
                        && let Ok(Some(bgm_name)) = database.get_bgm_link(index)
                    {
                        bgm_name_final = bgm_name;
                    } else {
                        bgm_name_final = bgm_link_opt.unwrap_or_default();
                    }
                    format!("{prefix}\n[BGM: \"{0}\"]\n", bgm_name_final)
                }
                "FadeIn" => {
                    let prefix = if is_all_script_instruction {
                        format!("{}\n", cap[0].to_string())
                    } else {
                        String::new()
                    };
                    format!("{prefix}\n-cutscene start")
                }
                "FadeOut" => {
                    let prefix = if is_all_script_instruction {
                        format!("{}\n", cap[0].to_string())
                    } else {
                        String::new()
                    };
                    format!("{prefix}\n-cutscene end")
                }
                "QuestAccepted" => {
                    let prefix = if is_all_script_instruction {
                        format!("{}\n", cap[0].to_string())
                    } else {
                        String::new()
                    };
                    format!("{prefix}\n-quest accepted")
                }
                "QuestCompleted" => {
                    let prefix = if is_all_script_instruction {
                        format!("{}\n", cap[0].to_string())
                    } else {
                        String::new()
                    };
                    format!("{prefix}\n-quest complete")
                }
                s if IGNORE_LOOKUP.contains(s) && !is_all_script_instruction => {
                    trim_after = true;
                    String::new()
                }
                _ => {
                    cap[0].to_string()
                }
            }
        });
        if trim_after {
            temp_line = replace_res.trim().to_string();
            trim_after = false;
        } else {
            temp_line = replace_res.to_string();
        }
        for cap in regex_self_var.captures_iter(&temp_line) {
            let (_, [param]) = cap.extract();
            let mut pv = None;
            if let Some(LuaValue::Integer(s)) = state.self_var_map.get(param) {
                pv = Some(s);
            } else if let Some(s) = state.database.quest_sys_define_map.get(param) {
                pv = Some(s);
            }
            if let Some(param_val) = pv {
                replaces.push((format!("self.{param}"), param_val.to_string()));
            }
        }
        for (from, to) in &replaces {
            temp_line = temp_line.replace(from.as_str(), to.as_str());
        }

        // HACK: Changing 'λ' back to ','
        temp_line = temp_line.replace('λ', ",");
        for (pre, rep) in temp_transform {
            temp_line = temp_line.replace(&rep, &pre);
        }
        if temp_line != "" {
            output.push(temp_line);
        }
    }
    Ok(output)
}

pub fn get_id(input_opt: &Option<String>) -> Option<String> {
    // TEXT_SUBWIL902_04780_NANANJI_000_002
    // TEXT, quest ID, quest number offset, and a name somewhere with the rest as numbers
    if let Some(input) = input_opt {
        let parts = input
            .strip_prefix("self.")
            .unwrap_or(input)
            .split('_')
            .filter(|s| !s.is_empty());

        // Looking over all quests, it's fine to skip the first two
        parts
            .skip(2)
            .find(|s| !s.chars().all(|c| c.is_ascii_digit()))
            .map(|s| s.to_string())
    } else {
        None
    }
}

fn find_closest_name(state: &mut State, key_name: &str, threshold: usize) -> Option<String> {
    let mut matched_name = None;
    let mut lowest = threshold;
    for (key, val) in &state.listener_key_to_name {
        let ld = levenshtein(key_name, key);
        if ld < lowest {
            matched_name = Some(val);
            lowest = ld;
        }
        if ld == 0 {
            // Exact match takes priority
            break;
        }
    }
    if let Some(matched) = matched_name {
        Some(matched.clone())
    } else {
        None
    }
}

fn get_lua_val(val: &str) -> Option<LuaValue> {
    if let Ok(val_i) = val.parse() {
        Some(LuaValue::Integer(val_i))
    }
    // else if let Ok(val_f) = val.parse() {
    //     Some(LuaValue::Float(val_f))
    // } else if val == "true" {
    //     Some(LuaValue::Bool)
    // } else if val == "false" {
    //     Some(LuaValue::Bool)
    // }
    else if val.starts_with("\"") && val.ends_with("\"") {
        Some(LuaValue::String(val[1..val.len() - 1].to_string()))
    } else if val.starts_with("function") {
        let arg_list: Vec<String> = val[9..val.len() - 1]
            .split(", ")
            .map(|x| x.trim().to_string())
            .collect();
        Some(LuaValue::Function(LuaFunction {
            args: arg_list,
            // var_map: HashMap::new(),
        }))
    }
    // else if val.starts_with("{") && val.starts_with("}")
    // {
    //     Some(LuaValue::Table(val.to_string()))
    // }
    else if val == "nil" {
        Some(LuaValue::Nil)
    } else {
        None
    }
}

fn get_self_var_name(key: &Option<String>) -> Option<String> {
    key.as_ref()
        .and_then(|name| match name.strip_prefix("self.") {
            Some(stripped) => Some(stripped.to_string()),
            None => Some(name.clone()),
        })
}

fn get_self_var_value(state: &State, key_opt: &Option<String>) -> Option<LuaValue> {
    if let Some(key) = get_self_var_name(key_opt) {
        if let Ok(f) = key.parse::<i32>() {
            return Some(LuaValue::Integer(f));
        } else if key.starts_with("\"") && key.ends_with("\"") {
            return Some(LuaValue::String(
                key.strip_circumfix("\"", "\"").unwrap().to_string(),
            ));
        } else if let Some(s) = state.self_var_map.get(&key) {
            return Some(s.clone());
        } else if let Some(s) = state.local_var_map.get(&key) {
            return Some(s.clone());
        } else if let Some(s) = state.database.quest_sys_define_map.get(&key) {
            return Some(LuaValue::Integer(*s));
        } else {
            return Some(LuaValue::String(key));
        }
    } else {
        None
    }
}

fn eval_func(database: &Database, state: &State, val: &str) -> Result<Option<LuaValue>> {
    let regex_call = Regex::new(r"\s*(\w+):(\w+)\(([^\)]*)\)")?;
    if let Ok(val_i) = val.parse() {
        Ok(Some(LuaValue::Integer(val_i)))
    }
    // else if let Ok(val_f) = val.parse() {
    //     Ok(Some(LuaValue::Float(val_f)))
    // } else if val == "true" {
    //     Ok(Some(LuaValue::Bool(true)))
    // } else if val == "false" {
    //     Ok(Some(LuaValue::Bool(false)))
    // }
    else if val.starts_with("\"") && val.ends_with("\"") {
        Ok(Some(LuaValue::String(val[1..val.len() - 1].to_string())))
    } else if val.starts_with("function") {
        let arg_list: Vec<String> = val[9..val.len() - 1]
            .split(", ")
            .map(|x| x.trim().to_string())
            .collect();
        Ok(Some(LuaValue::Function(LuaFunction {
            args: arg_list,
            // var_map: HashMap::new(),
        })))
    } else if val.starts_with("{") && val.ends_with("}") {
        let inner = val
            .strip_prefix('{')
            .and_then(|v| v.strip_suffix('}'))
            .unwrap_or_default()
            .trim();
        if inner == "" {
            return Ok(Some(LuaValue::Table(Vec::new())));
        }
        let split = inner
            .split(", ")
            .map(|s| s.trim().to_string())
            .collect::<Vec<String>>();
        Ok(Some(LuaValue::Table(split)))
    } else if val == "nil" {
        Ok(Some(LuaValue::Nil))
    } else if let Some(cap) = regex_call.captures(val) {
        let (_, [_, func, args]) = cap.extract();
        let mut arg_list = args.split(", ").map(|x| x.trim().to_string());
        match func {
            "BindCharacter" => {
                let level_link_opt = arg_list.next();
                if let Some(level_link_s) = &level_link_opt
                    && let Ok(level_i) = level_link_s.parse::<i32>()
                    && let Some(npc_name) = database.get_level_ref(level_i)?
                {
                    return Ok(Some(LuaValue::String(npc_name)));
                } else if let Some(LuaValue::Integer(level_link_i)) =
                    get_self_var_value(state, &level_link_opt)
                    && let Some(npc_name) = database.get_level_ref(level_link_i)?
                {
                    Ok(Some(LuaValue::String(npc_name)))
                } else {
                    Ok(None)
                }
            }
            "CreateCharacter" => {
                let listener_link_opt = arg_list.next();
                if let Some(listener_link_s) = &listener_link_opt
                    && let Ok(listener_i) = listener_link_s.parse::<i32>()
                    && let Some(npc_name) = database.get_name_from_id(listener_i)?
                {
                    Ok(Some(LuaValue::String(npc_name)))
                } else if let Some(LuaValue::Integer(listener_i)) =
                    get_self_var_value(state, &listener_link_opt)
                    && let Some(npc_name) = database.get_name_from_id(listener_i)?
                {
                    Ok(Some(LuaValue::String(npc_name.clone())))
                } else {
                    Ok(None)
                }
            }
            _ => Ok(None),
        }
    } else if let Some(v) = get_self_var_value(state, &Some(val.to_string())) {
        Ok(Some(v))
    } else {
        Ok(None)
    }
}

fn name_to_name_key(name: &str) -> String {
    name.chars()
        .filter(|x| x.is_alphabetic())
        .flat_map(|y| y.to_uppercase())
        .collect()
}

fn get_cutscene_text_by_index(
    ironworks: &Ironworks,
    excel: &Excel,
    database: &Database,
    i: i32,
    is_all_script_instruction: bool,
) -> Result<Vec<String>> {
    let Some(cut_path) = database.get_cutscene_path(i).ok().flatten() else {
        return Ok(Vec::new());
    };
    get_cutscene_text_by_path(
        ironworks,
        excel,
        database,
        &cut_path,
        is_all_script_instruction,
    )
}

fn get_cutscene_text_by_path(
    ironworks: &Ironworks,
    excel: &Excel,
    database: &Database,
    cut_path: &str,
    is_all_script_instruction: bool,
) -> Result<Vec<String>> {
    let Ok(cs) = CutsceneDialogue::from_path(ironworks, &format!("cut/{cut_path}.cutb")) else {
        return Ok(Vec::new());
    };

    let Ok(dialogue_list) = cs.get(database, excel, is_all_script_instruction) else {
        return Ok(Vec::new());
    };
    let mut filtered = vec![cut_path.to_string()];
    let mut last_was_newline = false;

    for s in dialogue_list {
        let is_newline = s.trim().is_empty();

        if is_newline {
            if !last_was_newline {
                filtered.push(s);
                last_was_newline = true;
            }
        } else {
            filtered.push(s);
            last_was_newline = false;
        }
    }
    Ok(filtered)
}
