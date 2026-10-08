use ironworks::sestring::MacroKind;
use ironworks::sestring::format::ColorUsage;
use ironworks::sestring::{Error as SeStringError, Expressions, format::Value};

use crate::cgwiki::color_category::ColorCategory;
use crate::cgwiki::lookup::sheet_lookup;
use crate::cgwiki::operator::operator;
use crate::{cgwiki::convert_value, cgwiki::from_expressions, cgwiki::state::CGWState};
use time::OffsetDateTime;
use time::ext::NumericalDuration;

pub fn bold(arguments: &mut Expressions, state: &mut CGWState) -> Result<(), SeStringError> {
    let enabled = from_expressions::take_u32(arguments, state)?;
    state
        .writer
        .set_style(ironworks::sestring::format::Style::Bold, enabled != 0)
}

pub fn italic(arguments: &mut Expressions, state: &mut CGWState) -> Result<(), SeStringError> {
    let enabled = from_expressions::take_u32(arguments, state)?;
    state
        .writer
        .set_style(ironworks::sestring::format::Style::Italic, enabled != 0)
}

pub fn edge(arguments: &mut Expressions, state: &mut CGWState) -> Result<(), SeStringError> {
    let enabled = from_expressions::take_u32(arguments, state)?;
    state
        .writer
        .set_style(ironworks::sestring::format::Style::Outline, enabled != 0)
}

pub fn shadow(arguments: &mut Expressions, state: &mut CGWState) -> Result<(), SeStringError> {
    let enabled = from_expressions::take_u32(arguments, state)?;
    state
        .writer
        .set_style(ironworks::sestring::format::Style::Shadow, enabled != 0)
}

pub fn color(
    kind: MacroKind,
    arguments: &mut Expressions,
    state: &mut CGWState,
) -> Result<(), SeStringError> {
    let tag = match kind {
        MacroKind::Color => "Color",
        MacroKind::ShadowColor => "ShadowColor",
        MacroKind::EdgeColor => "EdgeColor",
        _ => "",
    };

    let args = from_expressions::take_value(arguments, state)?;
    let arg_s: String;
    if let Value::U32(c) = args {
        let [_a, r, g, b] = c.to_be_bytes();
        arg_s = format!("#{:02X}{:02X}{:02X}", r, g, b);
    } else {
        arg_s = args.clone().into();
    }
    if arg_s.as_str() == "<StackColor>" {
        state.writer.write_str("</")?;
        state.writer.write_str(tag)?;
        state.writer.write_str(">")
    } else {
        state_write_single(state, tag, arg_s)
    }
}

pub fn color_type(arguments: &mut Expressions, state: &mut CGWState) -> Result<(), SeStringError> {
    let arg: Value = from_expressions::take_value(arguments, state)?;
    if let Value::U32(u) = arg {
        let category = ColorCategory::from(u);
        match category {
            ColorCategory::Unknown(_) => state_write_single(state, "ColorType", u.to_string()),
            _ => {
                if u == 0 {
                    state.writer.pop_color_type(ColorUsage::Foreground)
                } else {
                    state
                        .writer
                        .push_color_type(ColorUsage::Foreground, category)
                }
            }
        }
    } else {
        let arg_s: String = arg.clone().into();
        state_write_single(state, "ColorType", arg_s)
    }
}

pub fn edge_color_type(
    arguments: &mut Expressions,
    state: &mut CGWState,
) -> Result<(), SeStringError> {
    let arg: Value = from_expressions::take_value(arguments, state)?;
    if let Value::U32(u) = arg {
        let category = ColorCategory::from(u);
        match category {
            ColorCategory::Unknown(_) => state_write_single(state, "EdgeColorType", u.to_string()),
            _ => {
                if u == 0 {
                    state.writer.pop_color_type(ColorUsage::Edge)
                } else {
                    state.writer.push_color_type(ColorUsage::Edge, category)
                }
            }
        }
    } else {
        let arg_s: String = arg.clone().into();
        state_write_single(state, "EdgeColorType", arg_s)
    }
}

pub fn nbsp(_arguments: &mut Expressions, state: &mut CGWState) -> Result<(), SeStringError> {
    state.writer.write_str("&nbsp;")
}

pub fn hyphen(_arguments: &mut Expressions, state: &mut CGWState) -> Result<(), SeStringError> {
    state.writer.write_str("-")
}

pub fn soft_hyphen(
    _arguments: &mut Expressions,
    state: &mut CGWState,
) -> Result<(), SeStringError> {
    state.writer.write_str("\u{00AD}")
}

pub fn newline(_arguments: &mut Expressions, state: &mut CGWState) -> Result<(), SeStringError> {
    state.writer.write_str("<br>")
}

pub fn reset_time(arguments: &mut Expressions, state: &mut CGWState) -> Result<(), SeStringError> {
    let target_hour = from_expressions::take_u32(arguments, state)?;
    let target_weekday = from_expressions::try_take_u32(arguments, state)?;

    let time = state.time;
    let mut datetime = OffsetDateTime::from_unix_timestamp(time.into())
        .map_err(|_error| SeStringError::InvalidMacro)?;

    let mut day_offset = 0;

    // Get the offset required to reach the target weekday. If the target is in
    // the past this week, move to next week.
    if let Some(target_weekday) = target_weekday {
        let current_weekday = datetime.weekday().number_days_from_sunday();
        day_offset += i64::from(target_weekday) - i64::from(current_weekday);
        if day_offset < 0 {
            day_offset += 7;
        }
    }

    // If we've not moved forward on the day offset yet, and the target hour has
    // passed, move to the next day.
    if target_hour < datetime.hour().into() && day_offset <= 0 {
        day_offset += 1;
    }

    datetime += day_offset.days();
    datetime = datetime
        .replace_hour(target_hour.try_into().unwrap())
        .map_err(|_error| SeStringError::InvalidMacro)?;

    state.time = datetime.unix_timestamp().try_into().unwrap();

    Ok(())
}

pub fn set_time<'a>(
    arguments: &mut Expressions,
    state: &mut CGWState,
) -> Result<(), SeStringError> {
    let ts = from_expressions::take_value(arguments, state)?;
    match ts {
        Value::Unknown => {
            state.init_time();
        }
        x => {
            let ts: u32 = x.into();
            state.time = ts;
        }
    }
    Ok(())
}

pub fn r#if(
    if_type: &str,
    arguments: &mut Expressions,
    state: &mut CGWState,
) -> Result<(), SeStringError> {
    let arg_list = from_expressions::take_value_list(arguments, state)?;
    let cond: String = arg_list[0].clone().into();
    let a1: String = arg_list[1].clone().into();
    let a2: String = arg_list[2].clone().into();
    if a1 == a2 {
        state.writer.write_str(a1.as_str())
    } else if let Some(val) = cond.parse::<u32>().ok() {
        if val != 0 {
            state.writer.write_str(a1.as_str())
        } else {
            state.writer.write_str(a2.as_str())
        }
    } else if if_type == "If" {
        if let Some(comparison) = try_get_comparison(&cond) {
            match (
                comparison.0.as_str(),
                comparison.1.as_str(),
                comparison.2.as_str(),
                a1.as_str(),
                a2.as_str(),
            ) {
                (operator::GT, "<Global(52)>", "0", "[Limsa GC Title]", "") => {
                    state.writer.write_str("[Limsa GC Title]")
                }
                (operator::GT, "<Global(52)>", "0", _, _) => {
                    write_custom_if_tag(state, "IfLimsaGC", &a1, &a2)
                }
                (operator::GT, "<Global(53)>", "0", "[Gridania GC Title]", "") => {
                    state.writer.write_str("[Gridania GC Title]")
                }
                (operator::GT, "<Global(53)>", "0", _, _) => {
                    write_custom_if_tag(state, "IfGridaniaGC", &a1, &a2)
                }
                (operator::GT, "<Global(54)>", "0", "[Ul'dah GC Title]", "") => {
                    state.writer.write_str("[Ul'dah GC Title]")
                }
                (operator::GT, "<Global(54)>", "0", _, _) => {
                    write_custom_if_tag(state, "IfUldahGC", &a1, &a2)
                }
                (operator::EQ, "<Global(68)>", _, _, _) => {
                    if let Some(job_name) = comparison
                        .2
                        .parse::<u32>()
                        .ok()
                        .and_then(|f| sheet_lookup::get_class_job(state.excel, f))
                    {
                        write_custom_if_with_condition_tag(state, "IfClassJob", &job_name, &a1, &a2)
                    } else {
                        write_custom_if_with_condition_tag(
                            state,
                            "IfClassJob",
                            &comparison.2,
                            &a1,
                            &a2,
                        )
                    }
                }
                (
                    operator::EQ,
                    "<Global(80)>",
                    "0",
                    "<ColorType(546)>",
                    "<Icon2(15)> / <Icon2(8)>",
                ) => state.writer.write_str("{{cutscene interact controls}}"),
                _ => write_custom_if_with_condition_tag(state, &if_type, &cond, &a1, &a2),
            }
        } else {
            match cond.as_str() {
                "<Global(4)>" => state
                    .writer
                    .write_str(format!("{{{{alternatives|f={}|m={}}}}}", a1, a2).as_str()),
                "true" => state.writer.write_str(a1.as_str()),
                "false" => state.writer.write_str(a2.as_str()),
                _ => write_custom_if_with_condition_tag(state, &if_type, &cond, &a1, &a2),
            }
        }
    } else {
        write_custom_if_with_condition_tag(state, &if_type, &cond, &a1, &a2)
    }
}

pub fn switch(arguments: &mut Expressions, state: &mut CGWState) -> Result<(), SeStringError> {
    let arg_list = from_expressions::take_value_list(arguments, state)?;
    let a_sz: u32 = arg_list.len().try_into().unwrap_or_default();
    if let Value::String(s) = &arg_list[0] {
        match s.as_str() {
            "<Global(70)>" => {
                let a1: String = arg_list[1].clone().into();
                let a2: String = arg_list[2].clone().into();
                let a3: String = arg_list[3].clone().into();
                state.writer.write_str(
                    format!(
                        "{{{{alternatives|limsa={}|gridania={}|uldah={}}}}}",
                        a1, a2, a3
                    )
                    .as_str(),
                )
            }
            "<Global(71)>" => {
                let a1: String = arg_list[1].clone().into();
                let a2: String = arg_list[2].clone().into();
                let a3: String = arg_list[3].clone().into();
                let a4: String = arg_list[4].clone().into();
                let a5: String = arg_list[5].clone().into();
                let a6: String = arg_list[6].clone().into();
                let a7: String = arg_list[7].clone().into();
                let a8: String = arg_list[8].clone().into();
                state.writer.write_str(format!("{{{{alternatives|hyur={}|elezen={}|lalafell={}|miqote={}|roegadyn={}|aura={}|hrothgar={}|viera={}}}}}", a1, a2, a3, a4, a5, a6, a7, a8).as_str())
            }
            _ => {
                state.writer.write_str("<Switch(")?;
                state.writer.write_str(s.as_str())?;
                state.writer.write_str(")>")?;
                let mut index = 0;
                for arg in arg_list.iter().skip(1) {
                    let arg_s: String = arg.clone().into();
                    state.writer.write_str("<Case(")?;
                    state.writer.write(index.to_string())?;
                    state.writer.write_str(")>")?;
                    state.writer.write(arg_s)?;
                    state.writer.write_str("</Case>")?;
                    index = index + 1;
                }
                state.writer.write_str("</Switch>")
            }
        }
    } else if let Value::U32(u) = &arg_list[0]
        && *u < a_sz
    {
        let nth: String = arg_list.iter().nth(*u as usize).unwrap().clone().into();
        state.writer.write(nth)
    } else {
        let switch_var: String = arg_list[0].clone().into();
        state.writer.write_str("<Switch(")?;
        state.writer.write(switch_var)?;
        state.writer.write_str(")>")?;
        for (index, arg) in arg_list.iter().skip(1).enumerate() {
            let arg_s: String = arg.clone().into();
            state.writer.write_str("<Case(")?;
            state.writer.write(index.to_string())?;
            state.writer.write_str(")>")?;
            state.writer.write(arg_s)?;
            state.writer.write_str("</Case>")?;
        }
        state.writer.write_str("</Switch>")
    }
}

pub fn ruby(arguments: &mut Expressions, state: &mut CGWState) -> Result<(), SeStringError> {
    let text: String = from_expressions::take_str(arguments, state)?;
    let over: String = from_expressions::take_str(arguments, state)?;
    state.writer.write_str("<ruby>")?;
    state.writer.write_str(text.as_str())?;
    state.writer.write_str("<rp>(</rp><rt>")?;
    state.writer.write_str(over.as_str())?;
    state.writer.write_str("</rt><rp>)</rp>")?;
    state.writer.write_str("</ruby>")
}

pub fn split(arguments: &mut Expressions, state: &mut CGWState) -> Result<(), SeStringError> {
    let arg_list = from_expressions::take_value_list(arguments, state)?;
    let arg0: String = arg_list[0].clone().into();
    let arg1: String = arg_list[1].clone().into();
    let arg2: String = arg_list[2].clone().into();
    match (arg0.as_str(), arg1.as_str(), arg2.as_str()) {
        ("<Global(1)>", " ", "1") => state.writer.write_str("[Forename]"),
        ("<Global(1)>", " ", "2") => state.writer.write_str("[Surname]"),
        ("<PcName(<Local(1)>)>", " ", "1") => state.writer.write_str("<Forename(1)>"),
        ("<PcName(<Local(1)>)>", " ", "2") => state.writer.write_str("<Surname(1)>"),
        ("<PcName(<Local(2)>)>", " ", "1") => state.writer.write_str("<Forename(2)>"),
        ("<PcName(<Local(2)>)>", " ", "2") => state.writer.write_str("<Surname(2)>"),
        _ => {
            state.writer.write_str("<Split(")?;
            state.writer.write(convert_value::list_to_str(&arg_list)?)?;
            state.writer.write_str(")>")
        }
    }
}

pub fn sheet(arguments: &mut Expressions, state: &mut CGWState) -> Result<(), SeStringError> {
    let arg_list = from_expressions::take_value_list(arguments, state)?;

    if arg_list.len() == 3 {
        let sheet_name: String = arg_list[0].clone().into();
        let row_index: String = arg_list[1].clone().into();
        let column_index: u32 = arg_list[2].clone().into();
        let col_name: String = sheet_lookup::get_sheet_col(state.excel, &sheet_name, column_index)
            .unwrap_or(column_index.to_string());
        let replace: Option<String>;
        if let Ok(idx) = row_index.parse::<u32>() {
            let lookup_res = sheet_lookup::lookup_sheet(state.excel, &sheet_name, idx, &col_name);
            if let Some(x) = lookup_res {
                replace = Some(x)
            } else {
                replace = None
            }
        } else {
            replace = match (sheet_name.as_str(), row_index.as_str(), col_name.as_str()) {
                ("GCRankLimsaMaleText" | "GCRankLimsaFemaleText", "<Global(52)>", "NameRank") => {
                    Some("[Limsa GC Rank]".to_string())
                }
                ("GCRankLimsaMaleText" | "GCRankLimsaFemaleText", "<Global(52)>", "Singular") => {
                    Some("[Limsa GC Title]".to_string())
                }
                (
                    "GCRankGridaniaMaleText" | "GCRankGridaniaFemaleText",
                    "<Global(53)>",
                    "NameRank",
                ) => Some("[Gridania GC Rank]".to_string()),
                (
                    "GCRankGridaniaMaleText" | "GCRankGridaniaFemaleText",
                    "<Global(53)>",
                    "Singular",
                ) => Some("[Gridania GC Title]".to_string()),
                ("GCRankUldahMaleText" | "GCRankUldahFemaleText", "<Global(54)>", "NameRank") => {
                    Some("[Ul'dah GC Rank]".to_string())
                }
                ("GCRankUldahMaleText" | "GCRankUldahFemaleText", "<Global(54)>", "Singular") => {
                    Some("[Ul'dah GC Title]".to_string())
                }
                _ => None,
            };
        }

        match replace {
            Some(text) => state.writer.write(text),
            None => {
                // TODO: See why this sheet is giving unexpected value
                if sheet_name == "InstanceContent" {
                    write_sheet_tag(state, &sheet_name, &row_index, &column_index.to_string())
                } else {
                    write_sheet_tag(state, &sheet_name, &row_index, &col_name)
                }
            }
        }
    } else {
        state.writer.write_str("<Sheet(")?;
        for (i, value) in arg_list.iter().enumerate() {
            if i > 0 {
                state.writer.write_str(",")?;
            }
            let val_s: String = value.clone().into();
            state.writer.write(val_s)?;
        }
        state.writer.write_str(")>")
    }
}

pub fn string(arguments: &mut Expressions, state: &mut CGWState) -> Result<(), SeStringError> {
    // Always 1 arg
    let arg = from_expressions::take_str(arguments, state)?;
    state.writer.write(arg)
}

pub fn unknown(
    lang: u8,
    arguments: &mut Expressions,
    state: &mut CGWState,
) -> Result<(), SeStringError> {
    // 65 may be (Sheet, index, subIndex, column, xrefSheet, xrefSheetColumn)
    let name = format!("Macro_{}", lang);
    generic(&name, arguments, state)
}

fn write_sheet_tag(
    state: &mut CGWState,
    name: &str,
    row: &str,
    col: &str,
) -> Result<(), SeStringError> {
    state.writer.write_str("<Sheet(")?;
    state.writer.write_str(name)?;
    state.writer.write_str(",")?;
    state.writer.write_str(row)?;
    state.writer.write_str(",")?;
    state.writer.write_str(col)?;
    state.writer.write_str(")>")
}

fn write_custom_if_tag(
    state: &mut CGWState,
    if_type: &str,
    cond_true: &str,
    cond_false: &str,
) -> Result<(), SeStringError> {
    state.writer.write_str("<")?;
    state.writer.write_str(if_type)?;
    state.writer.write_str(">")?;
    state.writer.write_str(cond_true)?;
    if cond_false != "" {
        state.writer.write_str("<Else>")?;
        state.writer.write_str(cond_false)?;
    }
    state.writer.write_str("</")?;
    state.writer.write_str(if_type)?;
    state.writer.write_str(">")
}

fn write_custom_if_with_condition_tag(
    state: &mut CGWState,
    if_type: &str,
    cond: &str,
    cond_true: &str,
    cond_false: &str,
) -> Result<(), SeStringError> {
    state.writer.write_str("<")?;
    state.writer.write_str(if_type)?;
    state.writer.write_str("(")?;
    state.writer.write_str(cond)?;
    state.writer.write_str(")>")?;
    state.writer.write_str(cond_true)?;
    if cond_false != "" {
        state.writer.write_str("<Else>")?;
        state.writer.write_str(cond_false)?;
    }
    state.writer.write_str("</")?;
    state.writer.write_str(if_type)?;
    state.writer.write_str(">")
}

fn try_get_comparison(cond: &str) -> Option<(String, String, String)> {
    if let Some(op) = operator::VARS.iter().find(|o| cond.starts_with(*o)) {
        // Indices
        let after_paren_start = cond.find("(")? + 1;
        let comma_start = cond.find(",")?;
        let before_paren_end = cond.len() - 1;
        let arg1 = cond[after_paren_start..comma_start].to_string();
        let arg2 = cond[comma_start + 1..before_paren_end].to_string();
        let recs = (op.to_string(), arg1.clone(), arg2.clone());
        return Some(recs);
    } else {
        return None;
    }
}

fn state_write_single(state: &mut CGWState, tag: &str, arg: String) -> Result<(), SeStringError> {
    state.writer.write_str("<")?;
    state.writer.write_str(tag)?;
    state.writer.write_str("(")?;
    state.writer.write(arg)?;
    state.writer.write_str(")>")
}

pub fn generic(
    tag: &str,
    arguments: &mut Expressions,
    state: &mut CGWState,
) -> Result<(), SeStringError> {
    let arg_list = from_expressions::take_value_list(arguments, state)?;
    state.writer.write_str("<")?;
    state.writer.write_str(tag)?;
    state.writer.write_str("(")?;
    for (i, value) in arg_list.iter().enumerate() {
        if i > 0 {
            state.writer.write_str(",")?;
        }
        let val_s: String = value.clone().into();
        state.writer.write(val_s)?;
    }
    state.writer.write_str(")>")
}
