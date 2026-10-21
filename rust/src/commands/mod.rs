pub mod clean;
pub mod digest;
pub mod init;
pub mod mcp;
pub mod scan;
pub mod term;
pub mod tree;

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
    /// Bundle the whole project, or only the given paths, as one block.
    #[command(next_help_heading = "Share code")]
    Scan(scan::ScanArgs),
    /// Outline Python and MATLAB files: numbered signatures and docstrings, no bodies.
    #[command(next_help_heading = "Share code")]
    Digest(digest::DigestArgs),
    /// Print the project file tree.
    #[command(next_help_heading = "Share code")]
    Tree(tree::TreeArgs),
    /// Create .codigest/config.toml (file filters). `cdg mark` sets a baseline.
    #[command(next_help_heading = "Setup")]
    Init(init::InitArgs),
    /// Delete .codigest/: the config, the baseline and the cache.
    #[command(next_help_heading = "Setup")]
    Clean(clean::CleanArgs),
    /// Show recently captured commands and their output.
    #[command(next_help_heading = "Share a failing command")]
    Term(term::TermArgs),
    /// Serve these tools to AI agents over MCP (stdio).
    #[command(next_help_heading = "Agents")]
    Mcp,
}

/// Runs the parsed command and returns the process exit code.
pub fn run(cli: Cli) -> anyhow::Result<i32> {
    match cli.command {
        Command::Scan(args) => scan::run(args),
        Command::Digest(args) => digest::run(args),
        Command::Tree(args) => tree::run(args),
        Command::Init(args) => init::run(args),
        Command::Clean(args) => clean::run(args),
        Command::Term(args) => term::run(args),
        Command::Mcp => mcp::run(),
    }
}
