use std::{
    io::Write,
    path::{Path, PathBuf},
};

use clap::Parser;
use color_eyre::{owo_colors::OwoColorize, Result};

mod error;
mod luax;
mod server;

#[derive(Debug, Clone, clap::Parser)]
#[clap(about = "⛱️  A LuaX web framework")]
enum Args {
    #[clap(
        name = "serve",
        about = "Serve a directory of LuaX files in production mode"
    )]
    Serve {
        #[clap(
            short = 'C',
            long = "change-dir",
            default_value = ".",
            help = "The directory to serve LuaX files from"
        )]
        change_dir: std::path::PathBuf,
        #[clap(
            short = 'p',
            long = "port",
            default_value = "4310",
            help = "The port to serve on"
        )]
        port: u16,
        #[clap(
            short = 'l',
            long = "local",
            default_value = "false",
            help = "Do not use a temporary directory for preprocessing"
        )]
        local: bool,
    },
    #[clap(name = "build", about = "Build a directory of LuaX files")]
    Build {
        #[clap(
            short = 'C',
            long = "change-dir",
            default_value = ".",
            help = "The directory to build LuaX files from"
        )]
        change_dir: std::path::PathBuf,
        #[clap(
            short = 'o',
            long = "output",
            default_value = ".",
            help = "The directory to output the built files to"
        )]
        output_dir: std::path::PathBuf,
    },
    #[clap(
        name = "dev",
        about = "Serve a directory of LuaX files in development mode"
    )]
    Dev {
        #[clap(
            short = 'C',
            long = "change-dir",
            default_value = ".",
            help = "The directory to serve LuaX files from"
        )]
        change_dir: std::path::PathBuf,
        #[clap(
            short = 'P',
            long = "public-dir",
            default_value = ".",
            help = "The static files directory to serve"
        )]
        public_dir: std::path::PathBuf,
        #[clap(
            short = 'p',
            long = "port",
            default_value = "4310",
            help = "The port to serve on"
        )]
        port: u16,
        #[clap(
            short = 'l',
            long = "local",
            default_value = "false",
            help = "Do not use a temporary directory for preprocessing"
        )]
        local: bool,
    },
    #[clap(name = "new", about = "Create a new project")]
    New {
        #[clap(help = "The name of the project")]
        name: String,
    },
    #[clap(
        name = "init",
        about = "Initialize a new project in the current directory"
    )]
    Init,
}

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install().unwrap();
    let args = Args::parse();

    match args {
        Args::Serve {
            change_dir,
            port,
            local,
        } => {
            if !change_dir.is_dir() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("{} is not a directory", change_dir.display()),
                )
                .into());
            }

            println!("🌴 Project root: {}", change_dir.display().bright_yellow());

            if local {
                serve_locally(change_dir, false, port, None).await
            } else {
                serve_from_temp(change_dir, false, port, None).await
            }
        }
        Args::Build {
            change_dir,
            output_dir,
        } => build(change_dir, output_dir),
        Args::Dev {
            change_dir,
            public_dir,
            port,
            local,
        } => {
            if !change_dir.is_dir() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("{} is not a directory", change_dir.display()),
                )
                .into());
            }

            println!("🌴 Project root: {}", change_dir.display().bright_yellow());

            if !public_dir.is_dir() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("{} is not a directory", public_dir.display()),
                )
                .into());
            }

            println!(
                "🌴 Public directory: {}",
                public_dir.display().bright_yellow()
            );
            let public_dir = Some(public_dir.canonicalize()?);

            if local {
                serve_locally(change_dir, true, port, public_dir).await
            } else {
                serve_from_temp(change_dir, true, port, public_dir).await
            }
        }
        Args::New { name } => create_project(&name),
        Args::Init => init_project(),
    }
}

async fn serve_locally(
    change_dir: PathBuf,
    dev_mode: bool,
    port: u16,
    public_dir: Option<PathBuf>,
) -> Result<()> {
    println!("🌴 Running in local mode");
    std::env::set_current_dir(&change_dir)?;
    preprocess_current_dir().await?;

    ensure_entry_point().await?;

    serve(dev_mode, port, public_dir).await
}

async fn serve_from_temp(
    change_dir: PathBuf,
    dev_mode: bool,
    port: u16,
    public_dir: Option<PathBuf>,
) -> Result<()> {
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
        "⏲️  Will serve Lua from {}",
        tmp_dir.path().display().bright_blue()
    );

    let copied = recurse_copy_lua(&change_dir, tmp_dir.path())?;

    println!("⏲️  {} files copied", copied.bright_green());

    std::env::set_current_dir(tmp_dir.path())?;

    preprocess_current_dir().await?;

    ensure_entry_point().await?;

    serve(dev_mode, port, public_dir).await
}

async fn preprocess_current_dir() -> Result<()> {
    let current_dir = std::env::current_dir()?;
    let preprocessed = luax::preprocess_dir(current_dir.as_path(), current_dir.as_path())?;

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

async fn serve(dev_mode: bool, port: u16, public_dir: Option<PathBuf>) -> Result<()> {
    println!("📦 Building Lua state...");
    let lua = luax::prepare_lua(dev_mode)?;
    lua.context(|ctx| -> Result<()> {
        let entry_table: rlua::Table = ctx.load("require('reluax')").eval()?;
        let project_name: Option<String> = entry_table.get("name")?;

        if let Some(name) = project_name {
            println!("🌴 App name: {}", name.bright_yellow());
        }

        Ok(())
    })?;
    println!("🛫 Starting server on port {}...", port);
    server::Server::serve(lua, port, public_dir).await
}

fn recurse_copy_lua(from: &Path, to: &Path) -> Result<usize> {
    let mut copied = 0;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let path = entry.path();
        let file_name = path.file_name().unwrap();
        let to = to.join(file_name);

        if path.is_dir() {
            std::fs::create_dir(&to)?;
            copied += recurse_copy_lua(&path, &to)?;
        } else {
            let ext = path.extension().and_then(|s| s.to_str());
            if ext != Some("lua") && ext != Some("luax") {
                continue;
            }
            std::fs::copy(&path, &to)?;
            copied += 1;
        }
    }

    Ok(copied)
}

fn create_project(name: &str) -> Result<()> {
    let dir = PathBuf::from(name);

    if dir.is_dir() {
        println!("🛑 Directory {} already exists", dir.display().bright_red());
        return Ok(());
    }

    std::fs::create_dir(&dir)?;

    std::env::set_current_dir(&dir)?;

    write_templates(name)?;

    println!("🌴 Created project {}", name.bright_yellow());

    println!(
        "🛠️  To start a development server, change to the {} directory and run {}.",
        name.bright_yellow(),
        "reluax dev".bright_green()
    );

    Ok(())
}

fn init_project() -> Result<()> {
    let dir = std::env::current_dir()?;

    if !dir.is_dir() {
        println!("🛑 Directory {} does not exist", dir.display().bright_red());
        return Ok(());
    }

    let name = dir.file_name().unwrap().to_str().unwrap();

    write_templates(name)?;

    println!("🌴 Initialized project {}", name.bright_yellow());

    println!(
        "🛠️  To start a development server, run {}.",
        "reluax dev".bright_green()
    );

    Ok(())
}

fn write_templates(name: &str) -> Result<()> {
    let mut file = std::fs::File::create("reluax.luax")?;
    file.write_all(
        include_str!("../templates/reluax.luax")
            .replace("PROJECT_NAME", name)
            .as_bytes(),
    )?;

    let mut file = std::fs::File::create("index.luax")?;
    file.write_all(
        include_str!("../templates/index.luax")
            .replace("PROJECT_NAME", name)
            .as_bytes(),
    )?;

    let mut file = std::fs::File::create("style.css")?;
    file.write_all(include_str!("../templates/style.css").as_bytes())?;

    Ok(())
}

fn build(change_dir: PathBuf, output_dir: PathBuf) -> Result<()> {
    if !change_dir.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("{} is not a directory", change_dir.display()),
        )
        .into());
    }

    println!("🌴 Project root: {}", change_dir.display().bright_yellow());

    if !output_dir.is_dir() {
        std::fs::create_dir(&output_dir)?;
    }

    println!(
        "🌴 Output directory: {}",
        output_dir.display().bright_yellow()
    );

    std::env::set_current_dir(&change_dir)?;

    println!("📦 Preprocessing LuaX files...");

    let built = luax::preprocess_dir(&change_dir, &output_dir)?;

    println!("📦 {} LuaX files preprocessed!", built.bright_green());

    Ok(())
}
