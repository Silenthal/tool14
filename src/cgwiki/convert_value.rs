use ironworks::sestring::{Error as SeStringError, format::Value};
use time::OffsetDateTime;

use crate::cgwiki::state::CGWState;

pub fn into_time<T>(
    to_value: impl FnOnce(OffsetDateTime) -> T,
    state: &CGWState,
) -> Result<Value, SeStringError>
where
    T: TryInto<u32, Error: std::error::Error>,
{
    let datetime = OffsetDateTime::from_unix_timestamp(state.time.into())
        .map_err(|_err| SeStringError::InvalidExpression)?;

    Ok(Value::U32(
        to_value(datetime)
            .try_into()
            .expect("time conversion should not fail"),
    ))
}

pub fn into_global_param(value: Value) -> Result<Value, SeStringError> {
    /*
    Some global vals:
    1 = Player name
    2 = Target name
    52 = Limsa GC Rank
    53 = Gridania GC Rank
    54 = Ul'dah GC Rank
    55 = Chocobo name?
    68 = Class/job
    69 = Level of a non-combat class?
    72 = Level of a combat class?
    76 = Is legacy player
     */
    match value {
        Value::U32(u) => Ok(Value::String(format!("<Global({})>", u))),
        Value::String(s) => Ok(Value::String(format!("<Global_S(\"{}\")>", s))),
        Value::Unknown => Ok(Value::String("<Global_U(\"\")>".to_string())),
    }
}

pub fn into_local_param(value: Value) -> Result<Value, SeStringError> {
    match value {
        Value::U32(u) => Ok(Value::String(format!("<Local({})>", u))),
        Value::String(s) => Ok(Value::String(format!("<Local_S(\"{}\")>", s))),
        Value::Unknown => Ok(Value::String("<Local_U(\"\")>".to_string())),
    }
}

pub fn into_compare(
    comparator_str: &str,
    left: Value,
    right: Value,
) -> Result<Value, SeStringError> {
    if let Value::U32(left_num) = left
        && let Value::U32(right_num) = right
    {
        let res = match comparator_str {
            "GreaterThanOrEqualTo" => left_num >= right_num,
            "GreaterThan" => left_num > right_num,
            "LessThanOrEqualTo" => left_num <= right_num,
            "LessThan" => left_num < right_num,
            "Equal" => left_num == right_num,
            "NotEqual" => left_num != right_num,
            _ => false,
        };
        return Ok(Value::String(res.to_string()));
    }
    let left_str: String = left.into();

    let right_str: String = right.into();

    Ok(Value::String(format!(
        "{}({},{})",
        comparator_str, left_str, right_str
    )))
}

pub fn list_to_str(arg_list: &[Value]) -> Result<String, SeStringError> {
    Ok(arg_list
        .iter()
        .map(|x| -> String { x.clone().into() })
        .collect::<Vec<_>>()
        .join(","))
}
