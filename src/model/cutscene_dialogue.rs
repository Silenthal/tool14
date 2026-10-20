use std::{
    cmp::min,
    collections::{BTreeMap, HashMap},
    sync::LazyLock,
};

use anyhow::{Result, anyhow};
use binrw::BinRead;
use ironworks::{
    Ironworks,
    excel::Excel,
    file::{
        File,
        cutb::{Cutscene, Node},
        layer::{HelperKind, InstanceData, InstanceKind, Transform},
        tmb::{CommandKind, Condition, Item, Track},
    },
};
use regex::Regex;

use crate::{
    database::Database,
    detail_level::DetailLevel,
    model::{
        camera_binding::create_target_list,
        cutscene::cttl::{CTTL, TLCommand, TLCommandKind, TimelineItem},
        cutscene_simple::CutsceneSimple,
    },
    replace::get_id,
};

pub struct CutsceneDialogue {
    cutscene: Cutscene,
    ex_command_dict: HashMap<i16, TLCommand>,
}

impl CutsceneDialogue {
    pub fn from_path(ironworks: &Ironworks, cut_path: &str) -> Result<CutsceneDialogue> {
        let cutscene_bytes = ironworks.file::<Vec<u8>>(cut_path)?;
        let cutscene_stream = std::io::Cursor::new(cutscene_bytes);
        let cutscene = Cutscene::read(cutscene_stream)?;
        let cs_bytes_2 = ironworks.file::<Vec<u8>>(cut_path)?;
        let mut cs_stream_2 = std::io::Cursor::new(&cs_bytes_2);
        let cs_simple = CutsceneSimple::from_bytes(&mut cs_stream_2)?;
        let mut ex_command_dict = HashMap::new();
        for tag in cs_simple.tags {
            match &tag.magic.val {
                b"CTTL" => {
                    cs_stream_2.set_position(tag.item_offset as u64);
                    let cttl = CTTL::read(&mut cs_stream_2)?;
                    for tl_item in cttl.components {
                        match tl_item {
                            TimelineItem::Command(command_opt) => {
                                _ = ex_command_dict.insert(command_opt.id, command_opt)
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(CutsceneDialogue {
            cutscene,
            ex_command_dict,
        })
    }

    pub fn get(
        &self,
        database: &Database,
        excel: &Excel,
        detail_level: DetailLevel,
        arg_list: &Vec<(String, Option<String>)>,
    ) -> Result<Vec<String>> {
        let Some(Node::Sheet(dialogue_sheet_link)) = self
            .cutscene
            .nodes()
            .iter()
            .find(|s| matches!(s, Node::Sheet(_)))
        else {
            return Err(anyhow!("Unable to find linked sheet"));
        };
        let cutscene_dialogue_map = match database.get_text_map(excel, dialogue_sheet_link) {
            Ok(map) => map,
            Err(_) => HashMap::new(),
        };

        let Some(Node::Participants(participant_list)) = self
            .cutscene
            .nodes()
            .iter()
            .find(|s| matches!(s, Node::Participants(_)))
        else {
            return Err(anyhow!("No participants found"));
        };
        let mut participant_to_speaker: HashMap<u32, String> = HashMap::new();
        let mut participant_to_var = HashMap::new();
        for participant in participant_list
            .iter()
            .filter(|p| matches!(p.kind(), InstanceKind::HelperObject))
        {
            match participant.data() {
                InstanceData::HelperObject(help_obj) if help_obj.base_id() > 0 => {
                    if help_obj.kind() == HelperKind::EventNpc {
                        if let Some(npc_name) = database
                            .get_event_npc(help_obj.base_id().cast_signed())
                            .ok()
                            .flatten()
                        {
                            participant_to_speaker
                                .entry(participant.id())
                                .or_insert(npc_name);
                        }
                    } else if help_obj.kind() == HelperKind::Player {
                        participant_to_speaker.insert(participant.id(), "Player".to_string());
                    }
                }
                _ => {}
            }
        }
        let mut info_list: Vec<String> = Vec::new();
        let mut var_list: Vec<String> = Vec::new();

        for participant in participant_list {
            let p_id = participant.id() & 0xFFFFFF;
            let pos_str = get_pos(&participant.transform());
            let InstanceData::HelperObject(help_obj) = participant.data() else {
                continue;
            };
            let mut extra = String::new();
            let var_name;
            match help_obj.kind() {
                HelperKind::Player => {
                    participant_to_var.insert(participant.id(), "player".to_string());
                }
                HelperKind::EventNpc => {
                    if let Some(npc_name) = database
                        .get_event_npc(help_obj.base_id().cast_signed())
                        .ok()
                        .flatten()
                        && npc_name != ""
                    {
                        let npc_name_key: String = npc_name
                            .chars()
                            .filter(|c| c.is_alphanumeric())
                            .flat_map(|c| c.to_lowercase())
                            .collect();
                        var_name = format!("npc_{p_id:02}_{}", npc_name_key);
                        participant_to_var.insert(participant.id(), var_name.clone());
                        extra = format!("EventNPC {} ({:?})", var_name, help_obj);
                    } else {
                        let var_name = format!("npc_{p_id:02}");
                        participant_to_var.insert(participant.id(), var_name.clone());
                        extra = format!(
                            "EventNPC {} #{} ({:?})",
                            var_name,
                            help_obj.base_id(),
                            help_obj
                        );
                    }
                }
                HelperKind::Weapon => {
                    var_name = format!("weapon_{p_id:02}_{p_id:02}");
                    participant_to_var.insert(participant.id(), var_name.clone());
                    extra = format!("Weapon {} {:?}", var_name, help_obj.weapon());
                }
                HelperKind::None => {
                    var_name = format!("obj_{p_id:02}_{p_id:02}");
                    participant_to_var.insert(participant.id(), var_name.clone());
                    extra = format!("None {} ({:?})", var_name, help_obj);
                }
                _ => {
                    var_name = format!("obj_{p_id:02}_{p_id:02}");
                    participant_to_var.insert(participant.id(), var_name.clone());
                    extra = format!("Unk {} {:?}", var_name, participant.data());
                }
            }
            var_list.push(extra);
        }
        let regex_dialogue_alt_speaker = Regex::new(r"^\(-([^-]+)-\).*$")?;
        let mut temp_info_list = Vec::new();
        for node in self
            .cutscene
            .nodes()
            .iter()
            .filter(|s| matches!(s, Node::Timeline(_)))
        {
            let Node::Timeline(tl) = node else {
                continue;
            };
            temp_info_list.clear();
            let mut timeline_dict: BTreeMap<i16, Vec<(String, String, String)>> = BTreeMap::new();
            let mut command_dict = BTreeMap::new();
            let mut track_dict = BTreeMap::new();
            let mut actor_dict = BTreeMap::new();
            let mut timeline_header = String::new();
            let mut condition_str;
            let mut command_str;
            let mut dialogue_str;
            for item in tl.items() {
                match item {
                    Item::Header(header) => {
                        if detail_level == DetailLevel::FullScript {
                            timeline_header =
                                format!("\n-- Timeline, duration {}", header.duration());
                        }
                    }
                    Item::Actor(actor) => {
                        actor_dict.insert(actor.id(), actor);
                    }
                    Item::Track(track) => {
                        track_dict.insert(track.id(), track);
                    }
                    Item::Command(command) => {
                        command_dict.insert(command.id(), command);
                    }
                    _ => {}
                }
            }

            for (_, actor) in actor_dict {
                for (t_id, track) in track_dict
                    .iter()
                    .filter(|(t, _)| actor.tracks().contains(*t))
                {
                    for (_, command) in command_dict
                        .iter()
                        .filter(|(t, _)| track.commands().contains(*t))
                    {
                        let mut sort_key: u64 = 0;
                        let participant_var = participant_to_var
                            .get(&actor.participant())
                            .unwrap_or(&format!("P_{:08X}", &actor.participant()))
                            .clone();
                        condition_str = String::new();
                        dialogue_str = String::new();
                        match &command.kind() {
                            CommandKind::C048(cmd) => {
                                if let Some(speaker_key) = cmd.key()
                                    && let Some(speaker_val) =
                                        cutscene_dialogue_map.get(speaker_key)
                                {
                                    let final_speaker = match participant_to_speaker
                                        .get(&actor.participant())
                                        .and_then(|s| Some(s.clone()))
                                    {
                                        Some(s) => s,
                                        None => get_id(&Some(speaker_key.to_string()))
                                            .unwrap_or_default(),
                                    };
                                    let mut final_dialogue_line = speaker_val.clone();
                                    let mut alt_name_speaker = String::new();
                                    if let Some(caps) =
                                        regex_dialogue_alt_speaker.captures(&final_dialogue_line)
                                    {
                                        let (_, [alt_name]) = caps.extract();
                                        if alt_name != final_speaker {
                                            alt_name_speaker = format!("@{alt_name}");
                                        }
                                        final_dialogue_line = final_dialogue_line
                                            .strip_prefix(format!("(-{alt_name}-)").as_str())
                                            .unwrap_or(&final_dialogue_line)
                                            .to_string();
                                    }
                                    if final_dialogue_line.trim() == "" {
                                        final_dialogue_line = "<voiced line>".to_string();
                                    }
                                    dialogue_str = format!(
                                        "{}{}: {}",
                                        final_speaker, alt_name_speaker, final_dialogue_line
                                    );
                                }
                            }
                            CommandKind::Unknown { magic, body } if magic == b"C070" => {
                                let args = body
                                    .chunks(4)
                                    .into_iter()
                                    .map(|ll| u32::from_le_bytes(ll.try_into().unwrap()))
                                    .collect::<Vec<u32>>();
                                let Some(bgm_index) = args.get(4) else {
                                    break;
                                };
                                let Some(bgm_link) = database
                                    .get_bgm_link(bgm_index.cast_signed())
                                    .ok()
                                    .flatten()
                                else {
                                    break;
                                };
                                if detail_level != DetailLevel::NoScript {
                                    dialogue_str = format!("[BGM: \"{}\"]", bgm_link);
                                }
                            }
                            CommandKind::Unknown { magic, body } if magic == b"C098" => {
                                if let Some(ex_command) = self.ex_command_dict.get(&command.id())
                                    && let TLCommandKind::C098(c) = &ex_command.kind
                                {
                                    let question_key = if let Some(qt) = &c.question {
                                        qt.clone()
                                    } else {
                                        String::new()
                                    };
                                    let Some(question_text) =
                                        cutscene_dialogue_map.get(&question_key)
                                    else {
                                        break;
                                    };
                                    let mut answer_list = Vec::new();
                                    for answer_key in &c.answers {
                                        let Some(answer_text) =
                                            cutscene_dialogue_map.get(answer_key)
                                        else {
                                            break;
                                        };
                                        answer_list.push(answer_text.clone());
                                    }
                                    dialogue_str = format!("Question: {}", question_text);
                                    for a in answer_list {
                                        dialogue_str += &format!("\n> {}", a);
                                    }
                                }
                            }
                            _ => {}
                        }

                        command_str =
                            format!("    [Track {:03}, time {:>3}] ", t_id, command.time());
                        match &command.kind() {
                            CommandKind::C002(cmd) => {
                                sort_key = cmd.unknown_1() as u64;
                                let unk_lo = cmd.unknown_1() & 0xFFFF;
                                let unk_hi = (cmd.unknown_1() >> 16) & 0xFFFF;
                                let args = vec![format!(
                                    "path: \"{}\", duration: {}",
                                    cmd.path().unwrap_or_default(),
                                    cmd.duration()
                                )];
                                command_str += &format!(
                                    "[{:04X}, {:04X}] {}.PlayTimeline({})",
                                    unk_hi,
                                    unk_lo,
                                    participant_var,
                                    args.join(", ")
                                );
                            }
                            CommandKind::C004(cmd) => {
                                sort_key = cmd.unknown_1() as u64;
                                let unk_lo = cmd.unknown_1() & 0xFFFF;
                                let unk_hi = (cmd.unknown_1() >> 16) & 0xFFFF;
                                let near = cmd.near_plane();
                                let far = cmd.far_plane();
                                let mut args = vec![format!(
                                    "name: \"{}\", curve: {}, duration: {}",
                                    cmd.name().unwrap_or_default(),
                                    cmd.curve_id(),
                                    cmd.duration()
                                )];

                                {
                                    if (near - 0.1).abs() >= 1e-6 {
                                        args.push(format!("near: {near}"));
                                    }
                                    if (far - 1000.0).abs() >= 1e-6 {
                                        args.push(format!("far: {far}"));
                                    }
                                    let bind_list = create_target_list(cmd);
                                    let bind_str;
                                    if bind_list.iter().all(|t| t.target == 0xFFFFFFFF) {
                                        bind_str = "nil".to_string();
                                    } else {
                                        let mut targets = Vec::new();
                                        for mut part in bind_list {
                                            if part.target != 0xFFFFFFFF {
                                                let p_n = participant_to_var
                                                    .get(&part.target)
                                                    .unwrap_or(&format!("P_{:08X}", part.target))
                                                    .clone();
                                                part.target_name = Some(p_n);
                                            }
                                            targets.push(format!("{part}"));
                                        }
                                        let check = "nil".to_string();
                                        while targets.last() == Some(&check) {
                                            _ = targets.pop();
                                        }
                                        if targets.len() == 0 {
                                            bind_str = check.clone();
                                        } else {
                                            bind_str = format!("[{}]", targets.join(", "));
                                        }
                                    }
                                    args.push(format!("bindings: {}", bind_str));
                                }

                                command_str += &format!(
                                    "[{:04X}, {:04X}] {}.Camera({})",
                                    unk_hi,
                                    unk_lo,
                                    participant_var,
                                    args.join(", ")
                                );
                            }
                            CommandKind::C010(cmd) => {
                                sort_key = cmd.unknown_1() as u64;
                                let unk_lo = cmd.unknown_1() & 0xFFFF;
                                let unk_hi = (cmd.unknown_1() >> 16) & 0xFFFF;
                                let args = vec![format!(
                                    "name: \"{}\", flags: ${:X}, duration: {}, start: {}, end: {}",
                                    cmd.motion().unwrap_or_default(),
                                    cmd.flags(),
                                    cmd.duration(),
                                    cmd.animation_start(),
                                    cmd.animation_end()
                                )];
                                command_str += &format!(
                                    "[{:04X}, {:04X}] {}.Animation({})",
                                    unk_hi,
                                    unk_lo,
                                    participant_var,
                                    args.join(", ")
                                );
                            }
                            CommandKind::C013(cmd) => {
                                let unk_lo = cmd.unknown_2() & 0xFFFF;
                                let unk_hi = (cmd.unknown_2() >> 16) & 0xFFFF;
                                command_str += &format!(
                                    "[{:04X}, {:04X}] {}.SetAnimationCurve(curve: {}, duration: {}, placement: {})",
                                    unk_hi,
                                    unk_lo,
                                    participant_var,
                                    cmd.curve_id(),
                                    cmd.duration(),
                                    cmd.placement(),
                                );
                            }
                            CommandKind::C018(cmd) => {
                                sort_key = cmd.unknown_1() as u64;
                                let unk_lo = cmd.unknown_1() & 0xFFFF;
                                let unk_hi = (cmd.unknown_1() >> 16) & 0xFFFF;
                                command_str += &format!(
                                    "[{:04X}, {:04X}] {}.Transform({})",
                                    unk_hi,
                                    unk_lo,
                                    participant_var,
                                    get_pos_f(cmd.translation(), cmd.rotation(), cmd.scale())
                                );
                            }
                            CommandKind::C019(cmd) => {
                                sort_key = cmd.unknown_1() as u64;
                                let unk_lo = cmd.unknown_1() & 0xFFFF;
                                let unk_hi = (cmd.unknown_1() >> 16) & 0xFFFF;
                                command_str += &format!(
                                    "[{:04X}, {:04X}] {}.SetVisible(flag: ${:X}, duration: {})",
                                    unk_hi,
                                    unk_lo,
                                    participant_var,
                                    cmd.visibility(),
                                    cmd.duration()
                                );
                            }
                            CommandKind::C031(cmd) => {
                                sort_key = cmd.unknown_1() as u64;
                                let unk_lo = cmd.unknown_1() & 0xFFFF;
                                let unk_hi = (cmd.unknown_1() >> 16) & 0xFFFF;
                                command_str += &format!(
                                    "[{:04X}, {:04X}] {}.AnimationSummon(animation: {}, target_type: {}, duration: {})",
                                    unk_hi,
                                    unk_lo,
                                    participant_var,
                                    cmd.animation(),
                                    cmd.target_type(),
                                    cmd.duration()
                                );
                            }
                            CommandKind::C040(cmd) => {
                                sort_key = cmd.unknown_1() as u64;
                                let unk_lo = cmd.unknown_1() & 0xFFFF;
                                let unk_hi = (cmd.unknown_1() >> 16) & 0xFFFF;
                                command_str += &format!(
                                    "[{:04X}, {:04X}] {}.Animation(\"{}\")",
                                    unk_hi,
                                    unk_lo,
                                    participant_var,
                                    cmd.motion().unwrap_or_default()
                                );
                            }
                            CommandKind::C048(cmd) => {
                                sort_key = cmd.unknown_1() as u64;
                                let unk_lo = cmd.unknown_1() & 0xFFFF;
                                let unk_hi = (cmd.unknown_1() >> 16) & 0xFFFF;
                                command_str += &format!(
                                    "[{:04X}, {:04X}] {}.Dialogue(key: \"{}\", enabled: {}, captions: {:?}, unknown_3: {}, unknown_4: {}, unknown_5: {}, unknown_6: {}, unknown_7: {}, unknown_8: {}, unknown_9: {})",
                                    unk_hi,
                                    unk_lo,
                                    participant_var,
                                    cmd.key().unwrap_or_default(),
                                    cmd.enabled(),
                                    cmd.captions(),
                                    cmd.unknown_3(),
                                    cmd.unknown_4(),
                                    cmd.unknown_5(),
                                    cmd.unknown_6(),
                                    cmd.unknown_7(),
                                    cmd.unknown_8(),
                                    cmd.unknown_9()
                                );
                            }
                            CommandKind::C049(cmd) => {
                                sort_key = cmd.unknown_1() as u64;
                                let unk_lo = cmd.unknown_1() & 0xFFFF;
                                let unk_hi = (cmd.unknown_1() >> 16) & 0xFFFF;
                                let mut args = vec![format!(
                                    "path: \"{}\", curve: {}, enabled: {}, flags: ${:X}",
                                    cmd.path().unwrap_or_default(),
                                    cmd.curve_id(),
                                    cmd.enabled() != 0,
                                    cmd.flags()
                                )];
                                if cmd.bind_id_1() != 0xFFFF {
                                    args.push(format!(
                                        "bind_1: Bind({}, {})",
                                        cmd.bind_id_1(),
                                        cmd.bind_type_1()
                                    ));
                                }
                                if cmd.bind_id_2() != 0xFFFF {
                                    args.push(format!(
                                        "bind_1: Bind({}, {})",
                                        cmd.bind_id_2(),
                                        cmd.bind_type_2()
                                    ));
                                }
                                command_str += &format!(
                                    "[{:04X}, {:04X}] {}.VFX({})",
                                    unk_hi,
                                    unk_lo,
                                    participant_var,
                                    args.join(", ")
                                );
                            }
                            CommandKind::C063(cmd) => {
                                sort_key = cmd.unknown_1() as u64;
                                let unk_lo = cmd.unknown_1() & 0xFFFF;
                                let unk_hi = (cmd.unknown_1() >> 16) & 0xFFFF;
                                let mut args = vec![format!(
                                    "path: \"{}\", index: {}, loop: {}",
                                    cmd.path().unwrap_or_default(),
                                    cmd.sound_index(),
                                    cmd.loop_duration()
                                )];
                                if cmd.position_flags() != 0 {
                                    args.push(format!("pos_flags: ${:X}", cmd.position_flags()));
                                }
                                if cmd.bind_id() != 0 {
                                    args.push(format!("bind: ${:X}", cmd.bind_id()))
                                }
                                command_str += &format!(
                                    "[{:04X}, {:04X}] {}.SE({})",
                                    unk_hi,
                                    unk_lo,
                                    participant_var,
                                    args.join(", ")
                                );
                            }
                            CommandKind::C090(cmd) => {
                                sort_key = cmd.unknown_1() as u64;
                                let unk_lo = cmd.unknown_1() & 0xFFFF;
                                let unk_hi = (cmd.unknown_1() >> 16) & 0xFFFF;
                                command_str += &format!(
                                    "[{:04X}, {:04X}] {}.Expression(\"{}\")",
                                    unk_hi,
                                    unk_lo,
                                    participant_var,
                                    cmd.motion().unwrap_or_default()
                                );
                            }
                            CommandKind::C093(cmd) => {
                                sort_key = cmd.unknown_1() as u64;
                                let unk_lo = cmd.unknown_1() & 0xFFFF;
                                let unk_hi = (cmd.unknown_1() >> 16) & 0xFFFF;
                                command_str += &format!(
                                    "[{:04X}, {:04X}] {}.Color(color_1: {:?}, color_2: {:?}, duration: {})",
                                    unk_hi,
                                    unk_lo,
                                    participant_var,
                                    cmd.color_1(),
                                    cmd.color_2(),
                                    cmd.duration(),
                                );
                            }
                            CommandKind::C094(cmd) => {
                                sort_key = cmd.unknown_1() as u64;
                                let unk_lo = cmd.unknown_1() & 0xFFFF;
                                let unk_hi = (cmd.unknown_1() >> 16) & 0xFFFF;
                                let filter_str = if let Some(f) = cmd.filter() {
                                    f.filter().to_string()
                                } else {
                                    "nil".to_string()
                                };
                                command_str += &format!(
                                    "[{:04X}, {:04X}] {}.FadeControl(fade_time: {}, start: {}, end: {}, filter: {})",
                                    unk_hi,
                                    unk_lo,
                                    participant_var,
                                    cmd.fade_time(),
                                    cmd.start_visibility(),
                                    cmd.end_visibility(),
                                    filter_str,
                                );
                            }
                            CommandKind::C100(cmd) => {
                                let unk_lo = cmd.unknown_2() & 0xFFFF;
                                let unk_hi = (cmd.unknown_2() >> 16) & 0xFFFF;
                                command_str += &format!(
                                    "[{:04X}, {:04X}] {}.HideWeapon(visible: {}, unk3: {}, unk4: {})",
                                    unk_hi,
                                    unk_lo,
                                    participant_var,
                                    cmd.visibility(),
                                    cmd.unknown_3(),
                                    cmd.unknown_4(),
                                );
                            }
                            CommandKind::C107(cmd) => {
                                let unk_lo = cmd.unknown_2() & 0xFFFF;
                                let unk_hi = (cmd.unknown_2() >> 16) & 0xFFFF;
                                command_str += &format!(
                                    "[{:04X}, {:04X}] {}.VFXTrigger({})",
                                    unk_hi,
                                    unk_lo,
                                    participant_var,
                                    cmd.trigger_row(),
                                );
                            }
                            CommandKind::C114(cmd) => {
                                sort_key = cmd.unknown_1() as u64;
                                let unk_lo = cmd.unknown_1() & 0xFFFF;
                                let unk_hi = (cmd.unknown_1() >> 16) & 0xFFFF;
                                command_str += &format!(
                                    "[{:04X}, {:04X}] {}.WorldOriginInterpolate(...)",
                                    unk_hi, unk_lo, participant_var,
                                );
                            }
                            CommandKind::C118(cmd) => {
                                let unk_lo = cmd.unknown_2() & 0xFFFF;
                                let unk_hi = (cmd.unknown_2() >> 16) & 0xFFFF;
                                let args = vec![format!(
                                    "time: {}, unk_3: {}",
                                    cmd.transition_time(),
                                    cmd.unknown_3()
                                )];
                                command_str += &format!(
                                    "[{:04X}, {:04X}] {}.AnimationTransition({})",
                                    unk_hi,
                                    unk_lo,
                                    participant_var,
                                    args.join(", ")
                                );
                            }
                            CommandKind::C120(cmd) => {
                                let unk_lo = cmd.unknown_2() & 0xFFFF;
                                let unk_hi = (cmd.unknown_2() >> 16) & 0xFFFF;
                                command_str += &format!(
                                    "[{:04X}, {:04X}] {}.ControllerVibrate(duration: {}, wave: {})",
                                    unk_hi,
                                    unk_lo,
                                    participant_var,
                                    cmd.duration(),
                                    cmd.wave_type(),
                                );
                            }
                            CommandKind::C161(cmd) => {
                                let unk_lo = cmd.unknown_2() & 0xFFFF;
                                let unk_hi = (cmd.unknown_2() >> 16) & 0xFFFF;
                                command_str += &format!(
                                    "[{:04X}, {:04X}] {}.Blink({})",
                                    unk_hi,
                                    unk_lo,
                                    participant_var,
                                    cmd.blink()
                                );
                            }
                            CommandKind::Unknown { magic, body } => {
                                let chunks = body
                                    .chunks(4)
                                    .into_iter()
                                    .map(|ll| u32::from_le_bytes(ll.try_into().unwrap()))
                                    .collect::<Vec<u32>>();
                                let u1 = chunks
                                    .get(1)
                                    .and_then(|i| Some(i.clone()))
                                    .unwrap_or_default();
                                sort_key = u1 as u64;
                                let unk_lo = u1 & 0xFFFF;
                                let unk_hi = (u1 >> 16) & 0xFFFF;
                                let tag = String::from_utf8(magic.to_vec()).unwrap_or_default();
                                match tag.as_str() {
                                    "C039" => {
                                        let unk_dur = chunks
                                            .get(0)
                                            .and_then(|i| Some(i.clone()))
                                            .unwrap_or_default();
                                        let unk_1 = chunks
                                            .get(2)
                                            .and_then(|i| Some(i.clone()))
                                            .unwrap_or_default();
                                        let target_id = chunks
                                            .get(3)
                                            .and_then(|i| Some(i.clone()))
                                            .unwrap_or_default();
                                        let unk_2 = chunks
                                            .get(4)
                                            .and_then(|i| Some(i.clone()))
                                            .unwrap_or_default();
                                        let unk_f1 = chunks
                                            .get(5)
                                            .and_then(|i| Some(i.clone()))
                                            .unwrap_or_default();
                                        let unk_f2 = chunks
                                            .get(6)
                                            .and_then(|i| Some(i.clone()))
                                            .unwrap_or_default();
                                        let unk_3 = chunks
                                            .get(7)
                                            .and_then(|i| Some(i.clone()))
                                            .unwrap_or_default();
                                        let unk_4 = chunks
                                            .get(8)
                                            .and_then(|i| Some(i.clone()))
                                            .unwrap_or_default();
                                        let t_n = if target_id == 0 {
                                            String::new()
                                        } else {
                                            participant_to_var
                                                .get(&target_id)
                                                .unwrap_or(&format!("P_{:08X}", target_id))
                                                .clone()
                                        };

                                        let mut args = vec![format!(
                                            "target: \"{}\", duration: {}, f1: {}, f2: {}",
                                            t_n,
                                            unk_dur,
                                            f32::from_bits(unk_f1),
                                            f32::from_bits(unk_f2),
                                        )];
                                        if unk_1 != 1 {
                                            args.push(format!("unk_1: ${:08X}", unk_1));
                                        }
                                        if unk_2 != 0 {
                                            args.push(format!("unk_2: ${:08X}", unk_2));
                                        }
                                        if unk_3 != 2 {
                                            args.push(format!("unk_3: {}", unk_3));
                                        }
                                        if unk_4 != 0 {
                                            args.push(format!("unk_4: {}", unk_4));
                                        }
                                        command_str += &format!(
                                            "[{:04X}, {:04X}] {}.LookAt({})",
                                            unk_hi,
                                            unk_lo,
                                            participant_var,
                                            args.join(", ")
                                        );
                                    }
                                    "C070" => {
                                        let bgm_index = chunks
                                            .get(4)
                                            .and_then(|i| Some(i.clone()))
                                            .unwrap_or_default();
                                        let time = chunks
                                            .get(5)
                                            .and_then(|i| Some(i.clone()))
                                            .unwrap_or_default();
                                        let bgm_link = database
                                            .get_bgm_link(bgm_index.cast_signed())
                                            .ok()
                                            .flatten()
                                            .unwrap_or(bgm_index.to_string());

                                        command_str += &format!(
                                            "[{:04X}, {:04X}] {}.BGM(path: \"{}\", fade: {})",
                                            unk_hi, unk_lo, participant_var, bgm_link, time
                                        );
                                    }
                                    "C071" => {
                                        // obj_02.C071(00000001, 001A9DE5, 03E80005, 3F800000, 00000000)
                                        let unk_3 = chunks
                                            .get(2)
                                            .and_then(|i| Some(i.clone()))
                                            .unwrap_or_default();
                                        let unk_3_lo = unk_3 & 0xFFFF;
                                        let unk_3_hi = (unk_3 >> 16) & 0xFFFF;
                                        let unk_4 = chunks
                                            .get(3)
                                            .and_then(|i| Some(i.clone()))
                                            .unwrap_or_default();
                                        let args = format!(
                                            "{}, {}, {}",
                                            unk_3_lo,
                                            unk_3_hi,
                                            f32::from_bits(unk_4),
                                        );
                                        command_str += &format!(
                                            "[{:04X}, {:04X}] {}.{}({})",
                                            unk_hi, unk_lo, participant_var, tag, args
                                        );
                                    }
                                    "C098" => {
                                        if let Some(ex_command) =
                                            self.ex_command_dict.get(&command.id())
                                            && let TLCommandKind::C098(c) = &ex_command.kind
                                        {
                                            let q = if let Some(qt) = &c.question {
                                                qt.clone()
                                            } else {
                                                String::new()
                                            };
                                            command_str += &format!(
                                                "[{:04X}, {:04X}] {}.DialogueChoice(question: \"{}\", answers: {:?}, unknown_1: {}, unknown_2: {})",
                                                unk_hi,
                                                unk_lo,
                                                participant_var,
                                                q,
                                                c.answers,
                                                c.unknown_1,
                                                c.unknown_2
                                            );
                                        } else {
                                            let args = chunks
                                                .iter()
                                                .enumerate()
                                                .filter_map(|(i, l)| match i {
                                                    1 => None,
                                                    _ => Some(format!("{:08X}", l)),
                                                })
                                                .collect::<Vec<String>>()
                                                .join(", ");
                                            command_str += &format!(
                                                "[{:04X}, {:04X}] {}.{}({})",
                                                unk_hi, unk_lo, participant_var, tag, args
                                            );
                                        }
                                    }
                                    _ => {
                                        let args = chunks
                                            .iter()
                                            .enumerate()
                                            .filter_map(|(i, l)| match i {
                                                1 => None,
                                                _ => Some(format!("{:08X}", l)),
                                            })
                                            .collect::<Vec<String>>()
                                            .join(", ");
                                        command_str += &format!(
                                            "[{:04X}, {:04X}] {}.{}({})",
                                            unk_hi, unk_lo, participant_var, tag, args
                                        );
                                    }
                                }
                            }
                            _ => {
                                command_str += &format!("{}.{:?}", participant_var, command.kind());
                            }
                        }

                        let cond_base_list = track
                            .condition()
                            .iter()
                            .filter_map(|x| condition_to_text(x))
                            .collect::<Vec<String>>();
                        let cond_base;
                        cond_base = condition_list_narrow(track, arg_list)?.join(" ");
                        if cond_base != "" {
                            condition_str = format!("> if {cond_base}");
                        }

                        if condition_str != "" || command_str != "" || dialogue_str != "" {
                            let combo = (condition_str, command_str, dialogue_str);
                            // sort_key |= ((command.time() as u64) & 0xFFFFFFFF) << 32;
                            timeline_dict
                                .entry(command.time())
                                .and_modify(|m| m.push(combo.clone()))
                                .or_insert(vec![combo]);
                        }
                    }
                }
            }
            let mut last_cond = String::new();
            for (cond, comm, dial) in timeline_dict.values().flatten() {
                let include_cond =
                    detail_level == DetailLevel::FullScript || (cond.len() > 0 && dial != "");
                if last_cond.len() > 0 && cond == "" {
                    temp_info_list.push("---".to_string());
                }
                if include_cond && cond.len() > 0 {
                    temp_info_list.push(cond.clone());
                }
                if detail_level == DetailLevel::FullScript && comm.len() > 0 {
                    temp_info_list.push(comm.clone());
                }
                if dial.len() > 0 {
                    temp_info_list.push(dial.clone());
                }
                last_cond = if include_cond {
                    cond.clone()
                } else {
                    String::new()
                };
            }
            if last_cond.len() > 0 {
                temp_info_list.push("---".to_string());
            }
            if temp_info_list.len() > 0 {
                info_list.push(timeline_header);
                for line in &temp_info_list {
                    info_list.push(line.clone());
                }
            }
        }

        let mut out_list: Vec<String> = Vec::new();
        if info_list.len() > 0 {
            if detail_level == DetailLevel::FullScript {
                out_list.push("Vars:".to_string());
                out_list.extend(var_list);
            }
            out_list.extend(info_list);
        }
        Ok(out_list)
    }
}

fn condition_list_narrow(
    track: &Track,
    arg_list: &Vec<(String, Option<String>)>,
) -> Result<Vec<String>> {
    let mut output: Vec<String> = Vec::new();
    let mut temp_list: Vec<String> = Vec::new();
    let mut output_str: String = String::new();
    let mut paren_track = 0;
    for c in track.condition() {
        match c.operation() {
            0x1 => {
                paren_track += 1;
                output_str.push('(')
            }
            0x2 => {
                if temp_list.len() > 0 {
                    let pushed;
                    if temp_list[0].starts_with("var(1")
                        && let Some(s) = cond_expand(&temp_list, &arg_list)?
                    {
                        pushed = s;
                    } else {
                        pushed = temp_list.join(" ");
                    }
                    output_str.push_str(&pushed);
                    temp_list.clear();
                }
                if paren_track > 0 {
                    output_str.push(')');
                    paren_track -= 1;
                }
            }
            0x3..=0xD | 0x11.. => {
                temp_list.push(condition_to_text(&c).unwrap());
            }
            _ => output_str.push_str(&format!(" {} ", condition_to_text(&c).unwrap())),
        }
    }
    if temp_list.len() > 0 {
        let pushed;
        if let Some(s) = cond_expand(&temp_list, &arg_list)? {
            pushed = s;
        } else {
            pushed = temp_list.join(" ");
        }
        output_str.push_str(&pushed);
        temp_list.clear();
    }
    while paren_track > 0 {
        output_str.push(')');
        paren_track -= 1;
    }
    output_str = output_str
        .replace("(1)", "true")
        .replace("(!1)", "false")
        .replace("(0)", "false")
        .replace("(!0)", "true")
        .replace("(false)", "false")
        .replace("(true)", "true");
    if output_str.starts_with("false & ") {
        output_str = "false".to_string();
    }
    if output_str.starts_with("true | ") {
        output_str = output_str
            .strip_prefix("true | ")
            .unwrap_or(&output_str)
            .to_string();
    }
    Ok(vec![output_str])
}

static REGEX_VAR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"var\((.*), (.*)\)").unwrap());

static CLASSJOB_LIST: &[&str] = &[
    "adventurer",
    "gladiator",
    "pugilist",
    "marauder",
    "lancer",
    "archer",
    "conjurer",
    "thaumaturge",
    "carpenter",
    "blacksmith",
    "armorer",
    "goldsmith",
    "leatherworker",
    "weaver",
    "alchemist",
    "culinarian",
    "miner",
    "botanist",
    "fisher",
    "paladin",
    "monk",
    "warrior",
    "dragoon",
    "bard",
    "white mage",
    "black mage",
    "arcanist",
    "summoner",
    "scholar",
    "rogue",
    "ninja",
    "machinist",
    "dark knight",
    "astrologian",
    "samurai",
    "red mage",
    "blue mage",
    "gunbreaker",
    "dancer",
    "reaper",
    "sage",
    "viper",
    "pictomancer",
    "beastmaster",
];

fn cond_expand(
    temp_list: &Vec<String>,
    args: &Vec<(String, Option<String>)>,
) -> Result<Option<String>> {
    if temp_list.len() == 3 && temp_list[0].starts_with("var") && REGEX_VAR.is_match(&temp_list[0])
    {
        let m = temp_list[0].len() - 1;
        let first = temp_list[0].clone();
        let num = first[7..m].parse::<usize>()?;
        if num < 9 {
            return Ok(None);
        }
        let arg_index = num - 9;
        let value = temp_list[2].parse::<usize>()?;
        if num >= 14 {
            match (num, value) {
                (14, _) => Ok(Some(format!("dialogue choice {value}"))),
                (16, 1) | (17, 1) => Ok(Some("player:IsLegacy() == true".to_string())),
                (16, 0) | (17, 0) => Ok(Some("player:IsLegacy() == false".to_string())),
                (18, _) => {
                    if value < CLASSJOB_LIST.len() {
                        if temp_list[1] == "==" {
                            Ok(Some(format!(
                                "player:IsClassJob(\"{}\")",
                                CLASSJOB_LIST[value]
                            )))
                        } else {
                            Ok(Some(format!(
                                "!player:IsClassJob(\"{}\")",
                                CLASSJOB_LIST[value]
                            )))
                        }
                    } else {
                        Ok(None)
                    }
                }
                _ => Ok(None),
            }
        } else {
            match (temp_list[1].as_str(), value) {
                ("==", 0) | ("==", 1) | ("!=", 0) | ("!=", 1) => {
                    let mut is_val_arg = false;
                    let mut cond = match args.get(arg_index) {
                        Some((_, Some(s))) => {
                            if s.contains("GetAnimaWeaponQuestGender") {
                                is_val_arg = true;
                            }
                            s.clone()
                        }
                        Some((var_name, None)) => {
                            if let Ok(num_arg) = var_name.parse::<usize>() {
                                if temp_list[1] == "==" {
                                    return Ok(Some((num_arg == value).to_string()));
                                } else {
                                    return Ok(Some((num_arg != value).to_string()));
                                }
                            }
                            if var_name.contains("GetAnimaWeaponQuestGender")
                                | !var_name.contains(":")
                            {
                                is_val_arg = true;
                            }
                            var_name.clone()
                        }
                        _ => {
                            if arg_index == 0 {
                                if temp_list[1] == "==" {
                                    return Ok(Some((0 == value).to_string()));
                                } else {
                                    return Ok(Some((0 != value).to_string()));
                                }
                            } else {
                                return Ok(None);
                            }
                        }
                    };
                    if is_val_arg {
                        return Ok(Some(format!("{} {} {}", cond, temp_list[1], value)));
                    } else if value == 0 || (temp_list[1] == "!=" && value == 1) {
                        if cond.starts_with("!(") && cond.ends_with(")") {
                            cond = cond[2..cond.len() - 1].to_string();
                        } else if cond.contains(" and ")
                            || cond.contains(" or ")
                            || cond.contains("==")
                            || cond.contains("~=")
                        {
                            cond = format!("!({cond})");
                        } else {
                            cond = format!("!{cond}");
                        }
                    }
                    Ok(Some(cond))
                }
                _ => Ok(None),
            }
        }
    } else {
        return Ok(None);
    }
}

fn condition_to_text(cond: &Condition) -> Option<String> {
    let ff = match cond.operation() {
        0x1 => "(".to_string(),
        0x2 => ")".to_string(),
        0x3 => "+".to_string(),
        0x4 => "-".to_string(),
        0x5 => "*".to_string(),
        0x6 => "/".to_string(),
        0x7 => "%".to_string(),
        0x8 => ">".to_string(),
        0x9 => ">=".to_string(),
        0xA => "<".to_string(),
        0xB => "<=".to_string(),
        0xC => "!=".to_string(),
        0xD => "==".to_string(),
        0xE => "&".to_string(),
        0xF => "|".to_string(),
        0x10 => "!".to_string(),
        0x11 => cond.value().to_string(),
        0x12 => cond.float().to_string(),
        0x13 => {
            let pool = cond.value() >> 28;
            let var = cond.value() & 0x0FFFFFFF;
            if pool == 1 {
                match var {
                    0 => format!("var(game_lang)"),
                    1 => format!("var(caption_lang)"),
                    2 => format!("var(voice_lang)"),
                    3 => format!("var(skeleton_id)"),
                    4 => format!("var(start_town)"),
                    _ => format!("var({}, {})", pool, var),
                }
            } else {
                format!("var({}, {})", pool, var)
            }
        }
        0x14 => "True".to_string(),
        0x15 => "False".to_string(),
        0x16 => "abs".to_string(),
        0x17 => "acos".to_string(),
        0x18 => "asin".to_string(),
        0x19 => "atan2".to_string(),
        0x1A => "atan".to_string(),
        0x1B => "ceil".to_string(),
        0x1C => "cosh".to_string(),
        0x1D => "cos".to_string(),
        0x1E => "deg".to_string(),
        0x1F => "exp".to_string(),
        0x20 => "floor".to_string(),
        0x21 => "fmod".to_string(),
        0x22 => "frexp".to_string(),
        0x23 => "ldexp".to_string(),
        0x24 => "log10".to_string(),
        0x25 => "log".to_string(),
        0x26 => "max".to_string(),
        0x27 => "min".to_string(),
        0x28 => "modf".to_string(),
        0x29 => "pow".to_string(),
        0x2A => "rad".to_string(),
        0x2B => "random".to_string(),
        0x2C => "random_seed".to_string(),
        0x2D => "sinh".to_string(),
        0x2E => "sin".to_string(),
        0x2F => "sqrt".to_string(),
        0x30 => "tanh".to_string(),
        0x31 => "tan".to_string(),
        _ => String::new(),
    };
    if ff != "" { Some(ff) } else { None }
}

fn get_pos_f(t: [f32; 3], r: [f32; 3], s: [f32; 3]) -> String {
    let mut pos_arr = Vec::new();
    if !is_zero(t) {
        pos_arr.push(format!("translate: ({}, {}, {})", t[0], t[1], t[2]));
    }
    if !is_zero(r) {
        pos_arr.push(format!("rotate: ({}, {}, {})", r[0], r[1], r[2]));
    }
    if !is_one(s) {
        pos_arr.push(format!("scale: ({}, {}, {})", s[0], s[1], s[2]));
    }
    let mut pos_str = String::new();
    if pos_arr.len() > 0 {
        pos_str = format!("{}", pos_arr.join(", "));
    }
    pos_str
}

fn get_pos(tf: &Transform) -> String {
    let t = tf.translation();
    let r = tf.rotation();
    let s = tf.scale();
    get_pos_f(t, r, s)
}

fn is_zero(vec3: [f32; 3]) -> bool {
    let eps = 1e-6;
    let x2 = vec3[0].powi(2) + vec3[1].powi(2) + vec3[2].powi(2);
    x2 < eps
}

fn is_one(vec3: [f32; 3]) -> bool {
    let eps = 1e-6;
    (vec3[0] - 1.0).abs() < eps && (vec3[1] - 1.0).abs() < eps && (vec3[2] - 1.0).abs() < eps
}
