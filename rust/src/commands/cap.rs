//! `cap`: run a command transparently and record it for later recall.
//!
//! It declares no options of its own: everything after `cap` belongs to the wrapped
//! command, so flags like `-n` are never stolen from it.

use std::io::Write;

use clap::Args;

use crate::runner::{self, RunError};
use crate::style::{dim, red, yellow};
use crate::{shellhist, term};

#[derive(Args, Debug)]
#[command(trailing_var_arg = true)]
pub struct CapArgs {
    /// Command to run. Prefix with `--` so its flags are passed through.
    #[arg(value_name = "COMMAND...", num_args = 0.., allow_hyphen_values = true)]
    pub command: Vec<String>,
}

pub fn run(args: CapArgs) -> anyhow::Result<i32> {
    let command = args.command;
    if command.is_empty() {
        eprintln!(
            "{} cdg cap -- <command>\n{}",
            yellow("Usage:"),
            dim("Example: cdg cap -- pytest -q")
        );
        return Ok(2);
    }

    let command_text = runner::join_command(&command);
    let captured = match runner::run(&command) {
        Ok(captured) => captured,
        Err(RunError::NotFound) => {
            eprintln!("{} Command not found: {}", red("[Error]"), command[0]);
            return Ok(127);
        }
        Err(RunError::Launch(err)) => {
            eprintln!("{} Failed to launch command: {err}", red("[Error]"));
            return Ok(1);
        }
    };

    let entry = term::Entry {
        command: command_text,
        output: captured.output,
        exit_code: Some(i64::from(captured.exit_code)),
        cwd: std::env::current_dir()?.to_string_lossy().into_owned(),
        shell: shellhist::detect_shell(),
        // Taken after the run, as the Python build does.
        started_at: term::now_iso(),
        duration_ms: captured.duration_ms,
        ..term::Entry::default()
    };
    if let Err(err) = term::append_entry(entry, false) {
        eprintln!("{}", dim(&format!("Capture not stored: {err}")));
    }

    // The exit code is passed through whole: a Windows code such as -4058 does not fit
    // the `u8` the other commands return through `main`.
    let _ = std::io::stdout().flush();
    std::process::exit(captured.exit_code);
}
