use rlua::Lua;

use crate::{error::LuaXError, Result};

mod lexer;
mod preprocessor;
#[cfg(test)]
mod tests;
mod tokens;

pub fn table_to_html<W: std::io::Write>(table: rlua::Table, f: &mut W) -> Result<()> {
    let type_name: Option<String> = table.get("tag").unwrap();

    if type_name.is_none() {
        return Err(LuaXError::MissingField("tag".to_string()).into());
    }

    let type_name = type_name.unwrap();

    write!(f, "<{}", type_name)?;
    let mut children = None;
    let mut attrs = None;
    for pair in table.pairs::<String, rlua::Value>() {
        let (key, value) = pair?;
        if key == "tag" {
            continue;
        }
        if key == "children" {
            children = Some(value);
            continue;
        }
        if key == "attrs" {
            attrs = Some(value);
            continue;
        }
    }
    if let Some(attrs) = attrs {
        if let rlua::Value::Table(attrs) = attrs {
            for pair in attrs.pairs::<String, rlua::String>() {
                let (key, value) = pair?;
                write!(f, " {}=\"{}\"", key, value.to_str()?)?;
            }
        } else {
            return Err(LuaXError::NonTableAttrs.into());
        }
    }
    write!(f, ">")?;

    if let Some(children) = children {
        if let rlua::Value::Table(children) = children {
            for child in children.sequence_values::<rlua::Value>() {
                match child? {
                    rlua::Value::Table(child) => table_to_html(child, f)?,
                    rlua::Value::String(s) => write!(f, "{}", s.to_str()?)?,
                    _ => return Err(LuaXError::NonTableChildren.into()),
                }
            }
        } else {
            return Err(LuaXError::NonTableChildren.into());
        }
    }

    write!(f, "</{}>", type_name)?;

    Ok(())
}

pub fn preprocess(s: &str) -> Result<String> {
    let mut buf = Vec::new();
    let preprocessor = preprocessor::Preprocessor::new(s, &mut buf)?;

    preprocessor.preprocess()?;

    let s = String::from_utf8(buf).unwrap();

    Ok(s)
}

pub fn preprocess_dir(path: &std::path::Path) -> Result<usize> {
    let mut preprocessed = 0;
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            preprocessed += preprocess_dir(&path)?;
        } else {
            if path.extension().unwrap_or_default() != "luax" {
                continue;
            }
            let s = std::fs::read_to_string(&path)?;
            let s = preprocess(&s)?;

            let out_path = path.with_extension("lua");

            std::fs::write(out_path, s)?;
            preprocessed += 1;
        }
    }

    Ok(preprocessed)
}

pub fn prepare_lua() -> Result<Lua> {
    let lua = Lua::new();

    Ok(lua)
}
