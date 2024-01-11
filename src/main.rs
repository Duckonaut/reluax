use std::path::{Path, PathBuf};

use clap::Parser;
use color_eyre::{owo_colors::OwoColorize, Result};

mod error;
mod luax;
mod server;

#[derive(Debug, Clone, clap::Parser)]
struct Args {
    #[clap(short = 'C', long = "current-dir", default_value = ".")]
    current_dir: std::path::PathBuf,
    #[clap(short = 'p', long = "port", default_value = "4310")]
    port: u16,
    #[clap(short = 'l', long = "local", default_value = "false")]
    local: bool,
}

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install().unwrap();
    let args = Args::parse();

    if !args.current_dir.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("{} is not a directory", args.current_dir.display()),
        )
        .into());
    }

    println!(
        "🌴 Project root: {}",
        args.current_dir.display().bright_yellow()
    );

    if args.local {
        local(args).await
    } else {
        temp(args).await
    }
}

async fn local(args: Args) -> Result<()> {
    println!("🌴 Running in local mode");
    std::env::set_current_dir(&args.current_dir)?;
    preprocess_current_dir().await?;

    ensure_entry_point().await?;

    serve(args).await
}

async fn temp(args: Args) -> Result<()> {
    // Create a /tmp/reluax-XXXXXX directory for the server to pre-process files in.
    let tmp_dir = tempfile::Builder::new()
        .prefix("reluax-")
        .tempdir()
        .unwrap();

    if !tmp_dir.path().is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("{} is not a directory", tmp_dir.path().display()),
        )
        .into());
    }
    println!(
        "⏲️  Created temporary directory {}",
        tmp_dir.path().display().bright_blue()
    );

    println!(
        "⏲️  Will serve from {}",
        tmp_dir.path().display().bright_blue()
    );

    // copy all files from the current directory to the temporary directory recursively.
    let copied = recurse_copy(&args.current_dir, tmp_dir.path())?;

    println!("⏲️  {} files copied", copied.bright_green());

    std::env::set_current_dir(tmp_dir.path())?;

    preprocess_current_dir().await?;

    ensure_entry_point().await?;

    serve(args).await
}

async fn preprocess_current_dir() -> Result<()> {
    let preprocessed = luax::preprocess_dir(std::env::current_dir()?.as_path())?;

    println!(
        "⛱️  {} Reluax files preprocessed!",
        preprocessed.bright_green()
    );

    Ok(())
}

async fn ensure_entry_point() -> Result<()> {
    let entry = PathBuf::from("reluax.lua");

    if !entry.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("{} is not a file", entry.display()),
        )
        .into());
    }

    Ok(())
}

async fn serve(args: Args) -> Result<()> {
    server::Server::serve(args.port).await
}

fn recurse_copy(from: &Path, to: &Path) -> Result<usize> {
    let mut copied = 0;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let path = entry.path();
        let file_name = path.file_name().unwrap();
        let to = to.join(file_name);

        if path.is_dir() {
            std::fs::create_dir(&to)?;
            copied += recurse_copy(&path, &to)?;
        } else {
            std::fs::copy(&path, &to)?;
            copied += 1;
        }
    }

    Ok(copied)
}
