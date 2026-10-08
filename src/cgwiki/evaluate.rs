use crate::{
    cgwiki::convert_value, cgwiki::formatter::format_sestring, cgwiki::state::CGWState,
    cgwiki::writer::CGWikiWriter,
};
use ironworks::sestring::{Error as SeStringError, Expression, SeString, format::Value};
use time::OffsetDateTime;

pub fn evaluate_expression(
    expression: Expression,
    state: &CGWState,
) -> Result<Value, SeStringError> {
    let eval = |expr: Box<Expression>| evaluate_expression(*expr, state);

    let value = match expression {
        Expression::U32(value) => Value::U32(value),
        Expression::SeString(value) => evaluate_sestring(value.to_se_string(), state)?,

        Expression::Millisecond => convert_value::into_time(OffsetDateTime::millisecond, state)?,
        Expression::Second => convert_value::into_time(OffsetDateTime::second, state)?,
        Expression::Minute => convert_value::into_time(OffsetDateTime::minute, state)?,
        Expression::Hour => convert_value::into_time(OffsetDateTime::hour, state)?,
        Expression::Day => convert_value::into_time(OffsetDateTime::day, state)?,
        Expression::Weekday => {
            convert_value::into_time(|dt| dt.weekday().number_from_sunday(), state)?
        }
        Expression::Month => convert_value::into_time(|dt| u8::from(dt.month()), state)?,
        Expression::Year => convert_value::into_time(OffsetDateTime::year, state)?,

        Expression::StackColor => Value::String("<StackColor>".to_string()),

        Expression::GlobalNumber(inner) => convert_value::into_global_param(eval(inner)?)?,
        Expression::GlobalString(inner) => convert_value::into_global_param(eval(inner)?)?,
        Expression::LocalNumber(inner) => convert_value::into_local_param(eval(inner)?)?,
        Expression::LocalString(inner) => convert_value::into_local_param(eval(inner)?)?,

        Expression::Ge(left, right) => {
            convert_value::into_compare("GreaterThanOrEqualTo", eval(left)?, eval(right)?)?
        }
        Expression::Gt(left, right) => {
            convert_value::into_compare("GreaterThan", eval(left)?, eval(right)?)?
        }
        Expression::Le(left, right) => {
            convert_value::into_compare("LessThanOrEqualTo", eval(left)?, eval(right)?)?
        }
        Expression::Lt(left, right) => {
            convert_value::into_compare("LessThan", eval(left)?, eval(right)?)?
        }
        Expression::Eq(left, right) => {
            convert_value::into_compare("Equal", eval(left)?, eval(right)?)?
        }
        Expression::Ne(left, right) => {
            convert_value::into_compare("NotEqual", eval(left)?, eval(right)?)?
        }

        Expression::Unknown(unk) => Value::String(format!("M#{:02X}", unk)),
        _ => Value::Unknown,
    };

    Ok(value)
}

fn evaluate_sestring(sestring: SeString, state: &CGWState) -> Result<Value, SeStringError> {
    let mut writer = CGWikiWriter::new();
    let mut temp_state = CGWState::new(&mut writer, state.excel);
    temp_state.time = state.time;
    format_sestring(sestring, &mut temp_state)?;
    writer.reset_style();
    Ok(Value::String(writer.buffer))
}
