pub mod term;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "cdg",
    version,
    about = "Share the code context you choose with an AI: the project, a few files, what changed, or a failing command.",
    arg_required_else_help = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Show recently captured commands and their output.
    #[command(next_help_heading = "Share a failing command")]
    Term(term::TermArgs),
}

/// Runs the parsed command and returns the process exit code.
pub fn run(cli: Cli) -> anyhow::Result<i32> {
    match cli.command {
        Command::Term(args) => term::run(args),
    }
}
