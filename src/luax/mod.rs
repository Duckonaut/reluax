use std::path::Path;

use rlua::Lua;

use crate::{error::LuaXError, Result};

mod lexer;
mod preprocessor;
#[cfg(test)]
mod tests;
mod tokens;

pub fn table_to_html<W: std::io::Write>(table: rlua::Table, f: &mut W) -> Result<()> {
    let tag_name: Option<String> = table.get("tag").unwrap();

    if tag_name.is_none() {
        // we might be in a list
        for child in table.sequence_values::<rlua::Value>() {
            match child? {
                rlua::Value::Table(child) => table_to_html(child, f)?,
                rlua::Value::String(s) => write!(f, "{}", s.to_str()?)?,
                _ => return Err(LuaXError::NonTableChildren.into()),
            }
        }

        return Ok(());
    }

    let type_name = tag_name.unwrap();

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
                    rlua::Value::Boolean(b) => write!(f, "{}", b)?,
                    rlua::Value::Number(n) => write!(f, "{}", n)?,
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

pub fn table_to_json<W: std::io::Write>(table: rlua::Table, f: &mut W) -> Result<()> {
    let mut first = true;
    write!(f, "{{")?;
    for pair in table.pairs::<String, rlua::Value>() {
        let (key, value) = pair?;
        if !first {
            write!(f, ",")?;
        }
        first = false;
        write!(f, "\"{}\":", key)?;
        match value {
            rlua::Value::Table(t) => table_to_json(t, f)?,
            rlua::Value::String(s) => write!(f, "\"{}\"", s.to_str()?)?,
            rlua::Value::Boolean(b) => write!(f, "{}", b)?,
            rlua::Value::Number(n) => write!(f, "{}", n)?,
            rlua::Value::Nil => write!(f, "null")?,
            _ => return Err(LuaXError::NonJsonType.into()),
        }
    }
    write!(f, "}}")?;

    Ok(())
}

pub fn preprocess(s: &str) -> Result<String> {
    let mut buf = Vec::new();
    let preprocessor = preprocessor::Preprocessor::new(s, &mut buf)?;

    match preprocessor.preprocess() {
        Ok(_) => {}
        Err(e) => {
            println!("got up to: {}", String::from_utf8_lossy(&buf));
            return Err(e);
        }
    }

    let s = String::from_utf8(buf).unwrap();

    Ok(s)
}

pub fn preprocess_dir(path: &Path, output_path: &Path) -> Result<usize> {
    let mut preprocessed = 0;
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            let output_dir = output_path.join(path.file_name().unwrap());
            if !output_dir.is_dir() && output_dir.exists() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::AlreadyExists,
                    format!("{} already exists", output_dir.display()),
                )
                .into());
            }
            if !output_dir.exists() {
                std::fs::create_dir(&output_dir)?;
            }
            preprocessed += preprocess_dir(&path, &output_path.join(path.file_name().unwrap()))?;
        } else {
            if path.extension().unwrap_or_default() != "luax" {
                continue;
            }
            let s = std::fs::read_to_string(&path)?;
            let s = preprocess(&s)?;

            let out_path = output_path
                .join(path.file_name().unwrap())
                .with_extension("lua");

            std::fs::write(out_path, s)?;
            preprocessed += 1;
        }
    }

    Ok(preprocessed)
}

pub fn prepare_lua(dev_mode: bool) -> Result<Lua> {
    let lua = Lua::new();

    // create a table called "reluax" with common utility functions
    // and put it in the global scope
    lua.context(|ctx| -> Result<()> {
        let reluax = ctx.create_table()?;

        let url_matches = ctx.create_function(utils::url_matches)?;
        reluax.set("url_matches", url_matches)?;
        let url_extract = ctx.create_function(utils::url_extract)?;
        reluax.set("url_extract", url_extract)?;
        let html = ctx.create_function(utils::wrap_html)?;
        reluax.set("html", html)?;
        let html_page = ctx.create_function(utils::wrap_html_page)?;
        reluax.set("html_page", html_page)?;
        let json = ctx.create_function(utils::wrap_json)?;
        reluax.set("json", json)?;
        reluax.set("dev_mode", dev_mode)?;

        ctx.globals().set("reluax", reluax)?;

        Ok(())
    })?;

    Ok(lua)
}

mod utils {
    use rlua::{Context, Result, Table};

    /// Check if a path matches a pattern
    ///
    /// A pattern can contain the following:
    /// - `*` matches any number of characters
    /// - `{name}` matches anything except `/` and captures the value as `name`
    ///
    /// Example patterns:
    /// '/foo/bar'
    /// '/foo/{bar}' -> in a case of '/foo/baz' will capture 'baz' as 'bar'
    /// '/foo/*' -> in a case of '/foo/baz' will match
    ///
    /// This function only checks if the path matches the pattern, it does not
    /// capture any values.
    pub fn url_matches(_: Context<'_>, (pattern, path): (String, String)) -> Result<bool> {
        let mut path = path.chars();
        let mut pattern = pattern.chars();

        let mut path_char = path.next();
        let mut pattern_char = pattern.next();

        loop {
            if path_char.is_none() && (pattern_char.is_none() || pattern_char == Some('*')) {
                return Ok(true);
            } else if path_char == pattern_char {
                path_char = path.next();
                pattern_char = pattern.next();
            } else if pattern_char == Some('*') {
                pattern_char = pattern.next();
                if pattern_char.is_none() {
                    return Ok(true);
                }
                while path_char != pattern_char {
                    path_char = path.next();
                    if path_char.is_none() {
                        return Ok(false);
                    }
                }
            } else if pattern_char == Some('{') {
                pattern_char = pattern.next();
                let mut param_name = String::new();
                while pattern_char != Some('}') {
                    param_name.push(pattern_char.unwrap());
                    pattern_char = pattern.next();
                }
                pattern_char = pattern.next();
                while path_char != Some('/') && path_char.is_some() {
                    path_char = path.next();
                }
                if path_char.is_none() {
                    return Ok(true);
                }
            } else {
                return Ok(false);
            }
        }
    }

    /// Extract values from a path using a pattern
    ///
    /// A pattern can contain the following:
    /// - `*` matches any number of characters
    /// - `{name}` matches anything except `/` and captures the value as `name`
    ///
    /// This function will return a table with the captured values.
    pub fn url_extract(ctx: Context<'_>, (pattern, path): (String, String)) -> Result<Table> {
        let mut path = path.chars();
        let mut pattern = pattern.chars();

        let mut path_char = path.next();
        let mut pattern_char = pattern.next();

        let params = ctx.create_table()?;

        loop {
            if path_char.is_none() && (pattern_char.is_none() || pattern_char == Some('*')) {
                return Ok(params);
            } else if path_char == pattern_char {
                path_char = path.next();
                pattern_char = pattern.next();
            } else if pattern_char == Some('*') {
                pattern_char = pattern.next();
                if pattern_char.is_none() {
                    return Ok(params);
                }
                while path_char != pattern_char {
                    path_char = path.next();
                    if path_char.is_none() {
                        return Ok(params);
                    }
                }
            } else if pattern_char == Some('{') {
                pattern_char = pattern.next();
                let mut param_name = String::new();
                while pattern_char != Some('}') {
                    param_name.push(pattern_char.unwrap());
                    pattern_char = pattern.next();
                }
                pattern_char = pattern.next();
                let mut param_value = String::new();
                while path_char != Some('/') && path_char.is_some() {
                    param_value.push(path_char.unwrap());
                    path_char = path.next();
                }
                params.set(param_name, param_value)?;
                if path_char.is_none() {
                    return Ok(params);
                }
            } else {
                return Ok(params);
            }
        }
    }

    /// Wrap a table in a table to signal that it should be rendered as HTML
    pub fn wrap_html<'lua>(ctx: Context<'lua>, table: Table<'lua>) -> Result<Table<'lua>> {
        let html_table = ctx.create_table()?;
        html_table.set("type", "html")?;
        html_table.set("value", table)?;
        Ok(html_table)
    }

    /// Wrap a table in a table to signal that it should be rendered as a full HTML page
    pub fn wrap_html_page<'lua>(ctx: Context<'lua>, table: Table<'lua>) -> Result<Table<'lua>> {
        let html_table = ctx.create_table()?;
        html_table.set("type", "html-page")?;
        html_table.set("value", table)?;
        Ok(html_table)
    }

    /// Wrap a table in a table to signal that it should be rendered as JSON
    pub fn wrap_json<'lua>(ctx: Context<'lua>, table: Table<'lua>) -> Result<Table<'lua>> {
        let json_table = ctx.create_table()?;
        json_table.set("type", "json")?;
        json_table.set("value", table)?;
        Ok(json_table)
    }

    #[cfg(test)]
    mod tests {
        use rlua::Lua;

        #[test]
        fn url_matches() {
            let cases = vec![
                ("/", "/", true),
                ("/*/", "/abc/", true),
                ("/a/*/c", "/a/b/c", true),
                ("/a/*/c", "/a/b/c/d", false),
                ("/a/*/c", "/a/b", false),
                ("/a/*/c", "/a/f/c/d", false),
                ("/a", "/a", true),
                ("/a", "/b", false),
                ("/{name}", "/a", true),
                ("/{name}", "/a/b", false),
                ("/{name}/b", "/a/b", true),
                ("/{name}/b", "/a/c", false),
                ("/{name}/b", "/a/b/c", false),
            ];

            let lua = Lua::new();

            for (pattern, path, expected) in cases {
                let res: bool = lua
                    .context(|ctx| super::url_matches(ctx, (pattern.to_string(), path.to_string())))
                    .unwrap();
                if res != expected {
                    if expected {
                        panic!("expected {} to match {}", path, pattern);
                    } else {
                        panic!("expected {} to not match {}", path, pattern);
                    }
                }
            }
        }

        #[test]
        fn url_extract() {
            let cases = vec![
                ("/", "/", vec![]),
                ("/{name}", "/a", vec![("name", "a")]),
                ("/{name}/b", "/a/b", vec![("name", "a")]),
                ("/{name}/b", "/a/c", vec![]),
                ("/{name}/b", "/a/b/c", vec![]),
                (
                    "/{name}/b/{name2}",
                    "/a/b/c",
                    vec![("name", "a"), ("name2", "c")],
                ),
                (
                    "/{name}/b/{name2}",
                    "/a/b/c/d",
                    vec![("name", "a"), ("name2", "c")],
                ),
                ("/{name}/b/{name2}", "/a/b", vec![("name", "a")]),
            ];

            let lua = Lua::new();

            for (pattern, path, mut expected) in cases {
                expected.sort_by_key(|(k, _)| k.to_owned());
                lua.context(|ctx| {
                    let res =
                        super::url_extract(ctx, (pattern.to_string(), path.to_string())).unwrap();
                    let mut res_vec = Vec::new();
                    for pair in res.pairs::<String, String>() {
                        let (key, value) = pair.unwrap();
                        res_vec.push((key, value));
                    }
                    res_vec.sort_by_key(|(k, _)| k.clone());

                    let equal = res_vec
                        .iter()
                        .zip(expected.iter())
                        .all(|((k1, v1), (k2, v2))| k1 == k2 && v1 == v2);
                    if !equal {
                        panic!(
                            "expected {:?} to be {:?} for pattern {} and path {}",
                            res_vec, expected, pattern, path
                        );
                    }
                })
            }
        }
    }
}
