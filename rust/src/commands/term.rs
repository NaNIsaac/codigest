//! `term`: recall recently captured commands and their output.

use std::env;
use std::fs;
use std::path::Path;

use clap::{Args, Subcommand};

use super::{hook, ingest};
use crate::format::Format;
use crate::output::{Output, OutputArgs};
use crate::style::{dim, green, red, yellow};
use crate::{paths, shellhist, term};

#[derive(Args, Debug)]
#[command(args_conflicts_with_subcommands = true)]
pub struct TermArgs {
    #[command(subcommand)]
    pub action: Option<TermAction>,
    /// Number of recent commands to include
    #[arg(short = 'n', long, default_value_t = 1, value_parser = clap::value_parser!(u32).range(1..))]
    pub count: u32,
    /// Pick exactly one command, counting back from the latest (1 = last)
    #[arg(short, long, value_parser = clap::value_parser!(u32).range(1..))]
    pub index: Option<u32>,
    /// Only commands that exited non-zero
    #[arg(short, long)]
    pub failed: bool,
    /// Only commands run in the current directory or below
    #[arg(long)]
    pub here: bool,
    /// Read the native shell history instead of Codigest captures (no output)
    #[arg(long)]
    pub history_only: bool,
    /// Include the host name and shell recorded at capture time
    #[arg(long)]
    pub context: bool,
    /// Print only the stored output of one command, without tags or metadata
    #[arg(long, conflicts_with = "context")]
    pub raw: bool,
    /// Output format
    #[arg(long, value_enum, default_value_t = Format::Xml, conflicts_with = "raw")]
    pub format: Format,
    #[command(flatten)]
    pub out: OutputArgs,
}

#[derive(Subcommand, Debug)]
pub enum TermAction {
    /// Delete all stored terminal captures.
    Clear,
    /// Show where terminal captures are stored.
    Path,
    /// Hook your shells so every command's output is captured automatically.
    Install(hook::HookArgs),
    /// Remove the capture hook from your shell startup files.
    Uninstall(hook::HookArgs),
    /// Store one entry (called by the shell hook).
    #[command(hide = true)]
    Ingest(ingest::IngestArgs),
}

pub fn run(args: TermArgs) -> anyhow::Result<i32> {
    match args.action {
        Some(TermAction::Clear) => return clear(),
        Some(TermAction::Path) => {
            println!("{}", paths::history_path().display());
            return Ok(0);
        }
        Some(TermAction::Install(args)) => return hook::install(args),
        Some(TermAction::Uninstall(args)) => return hook::uninstall(args),
        Some(TermAction::Ingest(args)) => return ingest::run(args),
        None => {}
    }

    let out = Output::new(&args.out, true);
    // -i N needs N entries loaded so the Nth-from-last exists.
    let depth = args.index.unwrap_or(args.count) as usize;

    let mut entries = if args.history_only {
        Vec::new()
    } else {
        term::load_entries()
    };
    if entries.is_empty() {
        let (fallback, shell) = shellhist::read_recent(depth);
        if fallback.is_empty() {
            out.log(&yellow("No terminal history available."));
            diagnose(&out);
            return Ok(1);
        }
        out.log(&format!(
            "{} No Codigest captures found; falling back to {shell} history {}",
            yellow("[Warning]"),
            dim("(commands only, no output)")
        ));
        diagnose(&out);
        entries = fallback;
    }

    if args.failed || args.here {
        let cwd = env::current_dir()?;
        entries = term::select(entries, args.failed, args.here.then_some(cwd.as_path()));
        if entries.is_empty() {
            let what: Vec<&str> = [("failed", args.failed), ("in this directory", args.here)]
                .into_iter()
                .filter_map(|(w, on)| on.then_some(w))
                .collect();
            out.log(&yellow(&format!(
                "No captured commands match ({}).",
                what.join(" and ")
            )));
            return Ok(1);
        }
    }

    let selected: Vec<term::Entry> = match args.index {
        Some(index) => {
            let index = index as usize;
            if index > entries.len() {
                out.log(&format!(
                    "{} Only {} entries available; cannot reach index {index}.",
                    red("[Error]"),
                    entries.len()
                ));
                return Ok(1);
            }
            vec![entries[entries.len() - index].clone()]
        }
        None => entries[entries.len().saturating_sub(args.count as usize)..].to_vec(),
    };

    if args.raw {
        return raw(&out, &selected);
    }

    let content = match args.format {
        Format::Xml => term::render(&selected, args.context),
        Format::Markdown => term::render_markdown(&selected, args.context),
        Format::Json => term::render_json(&selected, args.context)?,
    };
    let item = if selected.len() == 1 {
        "1 terminal entry".to_string()
    } else {
        format!("{} terminal entries", selected.len())
    };
    out.deliver(&content, &item, |out| summarize(out, &selected))?;
    Ok(0)
}

/// The stored body of exactly one command. Several bodies run together would lose the
/// boundaries between them, so that is refused rather than guessed at.
fn raw(out: &Output, selected: &[term::Entry]) -> anyhow::Result<i32> {
    let [entry] = selected else {
        out.log(&format!(
            "{} --raw prints one command's output; {} were selected. Use -i N to pick one.",
            red("[Error]"),
            selected.len()
        ));
        return Ok(2);
    };
    if !entry.captured {
        out.log(&yellow(
            "No output stored for this command (read from shell history).",
        ));
        return Ok(1);
    }
    if !entry.note.is_empty() {
        out.log(&dim(&entry.note));
    }
    out.deliver(&entry.output, "output", |_| {})?;
    Ok(0)
}

fn summarize(out: &Output, entries: &[term::Entry]) {
    let total = entries.len();
    for (offset, entry) in entries.iter().enumerate() {
        let status = match entry.exit_code {
            None => String::new(),
            Some(0) => format!(" {}", green("ok")),
            Some(code) => format!(" {}", red(&format!("(exit {code})"))),
        };
        let marker = if entry.captured {
            String::new()
        } else {
            format!(" {}", yellow("(no output)"))
        };
        out.log(&format!(
            "{} {}{status}{marker}",
            dim(&format!("{}.", total - offset)),
            entry.command
        ));
    }
}

/// Explains why nothing was captured instead of failing silently.
fn diagnose(out: &Output) {
    if env::var_os("CODIGEST_HOOK").is_none() {
        out.log(&dim("Capture hook is not active in this shell. Run `cdg term install` and open a new window."));
    }
    let errors = paths::data_dir().join("term").join("ingest-errors.log");
    if errors.exists() {
        out.log(&format!(
            "{} {}",
            red("Capture errors were logged:"),
            errors.display()
        ));
    }
}

fn clear() -> anyhow::Result<i32> {
    let path = paths::history_path();
    let removed = if path.exists() {
        let count = term::load_from(&path).len();
        fs::remove_file(&path)?;
        count
    } else {
        0
    };
    let errors = path.with_file_name("ingest-errors.log");
    if Path::new(&errors).exists() {
        fs::remove_file(errors)?;
    }
    println!("{} {removed} captured entries.", green("Cleared"));
    Ok(0)
}
