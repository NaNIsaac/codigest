//! `init`: create `.codigest/config.toml`.

use std::fs;
use std::path::{Path, PathBuf};

use clap::Args;

use crate::common::hide_from_git;
use crate::pypath;
use crate::style::{dim, green, yellow};

const DEFAULT_CONFIG: &str = r#"# Codigest Configuration File
[project]
description = "Auto-generated context configuration"

[filter]
max_file_size_kb = 100
extensions = [
    ".py", ".pyi",
    ".ts", ".tsx", ".js", ".jsx",
    ".json", ".html", ".css",
    ".md", ".toml", ".yaml", ".yml", ".xml"
]
exclude_patterns = [
    "*.lock",
    "dist/",
    "build/",
    "node_modules/",
    "__pycache__/"
]

[output]
format = "xml"
structure = "toon"
"#;

#[derive(Args, Debug)]
pub struct InitArgs {
    /// Target directory to initialize
    #[arg(default_value = ".")]
    pub target: PathBuf,
    /// Overwrite existing config
    #[arg(short, long)]
    pub force: bool,
}

pub fn run(args: InitArgs) -> anyhow::Result<i32> {
    if !args.target.is_dir() {
        eprintln!(
            "Error: Invalid value for '[TARGET]': Directory '{}' does not exist.",
            args.target.display()
        );
        return Ok(2);
    }
    let root = pypath::resolve(&args.target);
    let config_dir = root.join(".codigest");
    let config_file = config_dir.join("config.toml");

    println!("Setting up Codigest in {}", root.display());

    hide_from_git(&config_dir)?;
    if !config_file.exists() || args.force {
        fs::write(&config_file, DEFAULT_CONFIG)?;
        println!("  {} .codigest/config.toml", green("Created"));
    } else {
        println!(
            "  {} .codigest/config.toml (--force overwrites it)",
            yellow("Kept")
        );
    }

    update_gitignore(&root.join(".gitignore"))?;

    println!("\nEdit .codigest/config.toml to choose which files are included.");
    println!(
        "{}",
        dim("To compare against this point later, run cdg mark.")
    );
    Ok(0)
}

fn update_gitignore(path: &Path) -> anyhow::Result<()> {
    let entry = ".codigest/";
    if !path.exists() {
        fs::write(path, format!("# Git Ignore\n{entry}\n"))?;
        return Ok(());
    }
    let text = fs::read_to_string(path)?;
    if !text.contains(entry) {
        // Appended to the file as it is: it may be CRLF already.
        let mut file = fs::OpenOptions::new().append(true).open(path)?;
        std::io::Write::write_all(&mut file, format!("\n{entry}\n").as_bytes())?;
        println!("  {} .codigest/ to .gitignore", green("Added"));
    }
    Ok(())
}
