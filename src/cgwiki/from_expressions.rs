use crate::{cgwiki::evaluate::evaluate_expression, cgwiki::state::CGWState};
use ironworks::sestring::{Error as SeStringError, Expressions, format::Value};

pub fn take_value(
    arguments: &mut Expressions,
    state: &mut CGWState,
) -> Result<Value, SeStringError> {
    let next_arg = arguments
        .next()
        .transpose()?
        .ok_or(SeStringError::InsufficientArguments)?;
    evaluate_expression(next_arg, state)
}

pub fn try_take_value(
    arguments: &mut Expressions,
    state: &mut CGWState,
) -> Result<Option<Value>, SeStringError> {
    let next_arg = arguments.next().transpose()?;
    match next_arg {
        Some(val) => {
            let value_wrap = evaluate_expression(val, state)?;
            Ok(Some(value_wrap))
        }
        None => Ok(None),
    }
}

pub fn take_str(
    arguments: &mut Expressions,
    state: &mut CGWState,
) -> Result<String, SeStringError> {
    let next_arg = arguments
        .next()
        .transpose()?
        .ok_or(SeStringError::InsufficientArguments)?;
    let value_wrap: Result<Value, SeStringError> = evaluate_expression(next_arg, state);
    match value_wrap {
        Ok(value) => match value {
            Value::String(s) => Ok(s),
            Value::U32(_u) => Err(SeStringError::InsufficientArguments),
            Value::Unknown => Err(SeStringError::InsufficientArguments),
        },
        Err(e) => Err(e),
    }
}

pub fn take_u32(arguments: &mut Expressions, state: &mut CGWState) -> Result<u32, SeStringError> {
    let next_arg = arguments
        .next()
        .transpose()?
        .ok_or(SeStringError::InsufficientArguments)?;
    let value_wrap: Result<Value, SeStringError> = evaluate_expression(next_arg, state);
    match value_wrap {
        Ok(value) => match value {
            Value::String(_s) => Err(SeStringError::InsufficientArguments),
            Value::U32(u) => Ok(u),
            Value::Unknown => Err(SeStringError::InsufficientArguments),
        },
        Err(e) => Err(e),
    }
}

pub fn try_take_u32(
    arguments: &mut Expressions,
    state: &mut CGWState,
) -> Result<Option<u32>, SeStringError> {
    let next_arg = arguments.next().transpose()?;
    match next_arg {
        Some(val) => {
            let value_wrap = evaluate_expression(val, state);
            match value_wrap {
                Ok(value) => match value {
                    Value::String(_) => Err(SeStringError::InsufficientArguments),
                    Value::U32(u) => Ok(Some(u)),
                    Value::Unknown => Err(SeStringError::InsufficientArguments),
                },
                Err(e) => Err(e),
            }
        }
        None => Ok(None),
    }
}

fn _write_val(next_arg: Value, state: &mut CGWState) -> Result<(), SeStringError> {
    match next_arg {
        Value::String(s) => state.writer.write_str(s.as_str())?,
        Value::U32(u) => state.writer.write_str(u.to_string().as_str())?,
        Value::Unknown => state.writer.write_str("<Unknown>")?,
    }
    Ok(())
}

pub fn take_value_list(
    arguments: &mut Expressions,
    state: &mut CGWState,
) -> Result<Vec<Value>, SeStringError> {
    let mut arg_list = vec![];
    while let Some(arg_val) = try_take_value(arguments, state)? {
        arg_list.push(arg_val);
    }
    Ok(arg_list)
}
