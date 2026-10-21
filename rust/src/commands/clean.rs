//! `clean`: delete `.codigest/`.

use std::path::PathBuf;

use clap::Args;

use crate::common::{ProjectContext, confirm, remove_tree};
use crate::pypath;
use crate::style::{bold, green, red, yellow};

#[derive(Args, Debug)]
pub struct CleanArgs {
    /// Target directory to clean
    #[arg(default_value = ".")]
    pub target: PathBuf,
    /// Skip confirmation prompt
    #[arg(short = 'y', long, visible_short_alias = 'f', visible_alias = "force")]
    pub yes: bool,
}

pub fn run(args: CleanArgs) -> anyhow::Result<i32> {
    if !args.target.is_dir() {
        eprintln!(
            "Error: Invalid value for '[TARGET]': Directory '{}' does not exist.",
            args.target.display()
        );
        return Ok(2);
    }
    let target = pypath::resolve(&args.target);
    let ctx = ProjectContext::new(std::slice::from_ref(&target))?;
    let codigest_dir = ctx.root_path.join(".codigest");

    if !codigest_dir.exists() {
        println!(
            "{}",
            yellow(&format!(
                "No .codigest directory found in {}",
                ctx.root_path.display()
            ))
        );
        return Ok(0);
    }

    if !args.yes {
        println!(
            "{}",
            yellow(&format!(
                "Warning: This will remove {} and all stored context/anchors.",
                bold(&codigest_dir.display().to_string())
            ))
        );
        match confirm("Are you sure you want to remove .codigest?") {
            Some(true) => {}
            Some(false) => {
                println!("{}", red("Aborted."));
                return Ok(0);
            }
            None => {
                eprintln!("Aborted!");
                return Ok(1);
            }
        }
    }

    match remove_tree(&codigest_dir) {
        Ok(()) => {
            println!("{} {}", green("Removed"), codigest_dir.display());
            Ok(0)
        }
        Err(err) => {
            println!("{} {err}", red("Error removing .codigest:"));
            Ok(1)
        }
    }
}
