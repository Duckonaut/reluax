use std::path::PathBuf;

use clap::Parser;
use color_eyre::Result;
use error::{LuaXError, ReluaxError};

mod error;
mod luax;
mod server;

fn table_to_html<W: std::io::Write>(table: rlua::Table, f: &mut W) -> Result<()> {
    let type_name: Option<String> = table.get("tag").unwrap();

    if type_name.is_none() {
        return Err(ReluaxError::LuaX(LuaXError::MissingField("tag".to_string())).into());
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
            return Err(ReluaxError::LuaX(LuaXError::NonTableAttrs).into());
        }
    }
    write!(f, ">")?;

    if let Some(children) = children {
        if let rlua::Value::Table(children) = children {
            for child in children.sequence_values::<rlua::Value>() {
                match child? {
                    rlua::Value::Table(child) => table_to_html(child, f)?,
                    rlua::Value::String(s) => write!(f, "{}", s.to_str()?)?,
                    _ => return Err(ReluaxError::LuaX(LuaXError::NonTableChildren).into()),
                }
            }
        } else {
            return Err(ReluaxError::LuaX(LuaXError::NonTableChildren).into());
        }
    }

    write!(f, "</{}>", type_name)?;

    Ok(())
}

#[derive(Debug, Clone, clap::Parser)]
struct Args {
    #[clap(short = 'C', long = "current-dir", default_value = ".")]
    current_dir: std::path::PathBuf,
    #[clap(short = 'p', long = "port", default_value = "4310")]
    port: u16,
}

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install().unwrap();
    let args = Args::parse();

    let current_dir = args.current_dir;

    if !current_dir.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("{} is not a directory", current_dir.display()),
        )
        .into());
    }

    std::env::set_current_dir(current_dir)?;

    luax::preprocess_dir(std::env::current_dir()?.as_path())?;

    println!("⛱️  Reluax files preprocessed!");

    let entry = PathBuf::from("reluax.lua");

    if !entry.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("{} is not a file", entry.display()),
        )
        .into());
    }

    server::Server::serve(args.port).await?;

    Ok(())
}
