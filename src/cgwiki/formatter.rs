use crate::{cgwiki::state::CGWState, cgwiki::writer::CGWikiWriter};
use ironworks::{
    excel::Excel,
    sestring::{Error as SeStringError, MacroKind, MacroPayload, Payload, SeString},
};

use crate::{cgwiki::handle_macro, cgwiki::string_process};

pub fn format_string(excel: &Excel, sestring: &SeString) -> String {
    let mut writer = CGWikiWriter::new();

    let mut state = CGWState::new(&mut writer, excel);

    for payload in sestring.payloads() {
        match payload {
            Ok(Payload::Text(inner)) => {
                let s = inner.as_utf8();
                match s {
                    Ok(ss) => {
                        _ = state.writer.write_str(&ss);
                    }
                    Err(_e) => {
                        _ = state.writer.write_str("<Error: unicode decode fail>");
                    }
                }
            }
            Ok(Payload::Macro(inner)) => {
                let ef = inner.kind().clone();
                let res = format_macro(inner, &mut state);
                if let Err(e) = res {
                    let es = format!("<Error: macro decode fail ({}, {:?})>", e, ef);
                    _ = state.writer.write_str(&es);
                }
            }
            Err(e) => {
                let f = format!("<Error: {}>", e);
                _ = state.writer.write_str(&f);
            }
        }
    }

    writer.reset_style();
    return String::from(string_process::clean_str(&writer.buffer));
}

pub fn format_sestring(sestring: SeString, state: &mut CGWState) -> Result<(), SeStringError> {
    for payload in sestring.payloads() {
        match payload? {
            Payload::Text(inner) => state.writer.write_str(inner.as_utf8()?)?,
            Payload::Macro(inner) => format_macro(inner, state)?,
        }
    }

    Ok(())
}

fn format_macro(payload: MacroPayload, state: &mut CGWState) -> Result<(), SeStringError> {
    let mut arguments = payload.expressions();

    match payload.kind() {
        MacroKind::Bold => handle_macro::bold(&mut arguments, state),
        MacroKind::Italic => handle_macro::italic(&mut arguments, state),
        MacroKind::Edge => handle_macro::edge(&mut arguments, state),
        MacroKind::Shadow => handle_macro::shadow(&mut arguments, state),

        MacroKind::Color => handle_macro::color(payload.kind(), &mut arguments, state),
        MacroKind::EdgeColor => handle_macro::color(payload.kind(), &mut arguments, state),
        MacroKind::ShadowColor => handle_macro::color(payload.kind(), &mut arguments, state),

        MacroKind::ColorType => handle_macro::color_type(&mut arguments, state),
        MacroKind::EdgeColorType => handle_macro::edge_color_type(&mut arguments, state),

        MacroKind::NonBreakingSpace => handle_macro::nbsp(&mut arguments, state),
        MacroKind::Hyphen => handle_macro::hyphen(&mut arguments, state),
        MacroKind::SoftHyphen => handle_macro::soft_hyphen(&mut arguments, state),
        MacroKind::NewLine => handle_macro::newline(&mut arguments, state),
        // Reference to the Platform sheet with an index plus a number based on the platform
        MacroKind::SwitchPlatform => handle_macro::generic("PlatformStr", &mut arguments, state),

        MacroKind::SetResetTime => handle_macro::reset_time(&mut arguments, state),
        MacroKind::SetTime => handle_macro::set_time(&mut arguments, state),

        MacroKind::If => handle_macro::r#if("If", &mut arguments, state),
        MacroKind::IfSelf => handle_macro::r#if("IfSelf", &mut arguments, state),
        MacroKind::IfPcGender => handle_macro::r#if("IfPcGender", &mut arguments, state),
        MacroKind::IfPcName => handle_macro::r#if("IfPcName", &mut arguments, state),

        MacroKind::Switch => handle_macro::switch(&mut arguments, state),

        // Sheet, Article?, Index, plural_name, singular_name
        // 3 = none
        // 2 = the
        // 1 = a/n
        // "a" = a?
        MacroKind::JaNoun => handle_macro::generic("JaNoun", &mut arguments, state),
        MacroKind::EnNoun => handle_macro::generic("EnNoun", &mut arguments, state),
        MacroKind::DeNoun => handle_macro::generic("DeNoun", &mut arguments, state),
        MacroKind::FrNoun => handle_macro::generic("FrNoun", &mut arguments, state),
        MacroKind::ChNoun => handle_macro::generic("ChNoun", &mut arguments, state),
        MacroKind::Ruby => handle_macro::ruby(&mut arguments, state),

        MacroKind::Icon => handle_macro::generic("Icon", &mut arguments, state),
        MacroKind::Icon2 => handle_macro::generic("Icon2", &mut arguments, state),

        // Note: Used in the quest "Clotted Crime"
        MacroKind::Caps => handle_macro::generic("Caps", &mut arguments, state),
        MacroKind::Head => handle_macro::generic("Head", &mut arguments, state),
        MacroKind::HeadAll => handle_macro::generic("HeadAll", &mut arguments, state),
        MacroKind::LowerHead => handle_macro::generic("LowerHead", &mut arguments, state),
        MacroKind::Lower => handle_macro::generic("Lower", &mut arguments, state),
        MacroKind::Split => handle_macro::split(&mut arguments, state),

        MacroKind::Kilo => handle_macro::generic("Kilo", &mut arguments, state),
        MacroKind::Digit => handle_macro::generic("Digit", &mut arguments, state),
        MacroKind::Float => handle_macro::generic("Float", &mut arguments, state),

        // Key may be used for "popups" when a page break is needed
        // The Key with numbers *might* be VO lines, but most aren't
        MacroKind::Key => handle_macro::generic("Key", &mut arguments, state),
        MacroKind::Sec => handle_macro::generic("Sec", &mut arguments, state),

        MacroKind::LevelPos => handle_macro::generic("LevelPos", &mut arguments, state),
        MacroKind::Ordinal => handle_macro::generic("Ordinal", &mut arguments, state),
        MacroKind::Fixed => handle_macro::generic("Fixed", &mut arguments, state),

        // Sheet(sheet_name, row, column_index_after_first)
        MacroKind::Sheet => handle_macro::sheet(&mut arguments, state),
        MacroKind::SheetSub => handle_macro::generic("SheetSub", &mut arguments, state),
        MacroKind::String => handle_macro::string(&mut arguments, state),
        MacroKind::Num => handle_macro::generic("Num", &mut arguments, state),
        MacroKind::PcName => handle_macro::generic("PcName", &mut arguments, state),
        MacroKind::Sound => handle_macro::generic("Sound", &mut arguments, state),
        MacroKind::Unknown(x) => handle_macro::unknown(x, &mut arguments, state),
        _ => {
            println!("Unhandled macro {:?}", payload.kind());
            Ok(())
        }
    }
}
