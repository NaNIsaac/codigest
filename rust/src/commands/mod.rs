pub mod cap;
pub mod changes;
pub mod clean;
pub mod digest;
pub mod hook;
pub mod ingest;
pub mod init;
pub mod mcp;
pub mod pack;
pub mod scan;
pub mod status;
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

// Listed in the order `cdg --help` shows them in the Python build.
#[derive(Subcommand, Debug)]
pub enum Command {
    /// Bundle the whole project, or only the given paths, as one block.
    Scan(scan::ScanArgs),
    /// Outline Python and MATLAB files: numbered signatures and docstrings, no bodies.
    Digest(digest::DigestArgs),
    /// Print the project file tree.
    Tree(tree::TreeArgs),
    /// Set the baseline that `diff`, `semdiff` and `pack` compare against.
    Mark(changes::MarkArgs),
    /// Show the text diff since the baseline (`cdg mark`).
    Diff(changes::DiffArgs),
    /// Show which Python functions and classes changed since the baseline.
    Semdiff(changes::SemdiffArgs),
    /// Run a command as usual and keep its output for `pack` and `term`.
    Cap(cap::CapArgs),
    /// Bundle the last failure, the files it points at and your changes.
    Pack(pack::PackArgs),
    /// Show recently captured commands and their output.
    Term(term::TermArgs),
    /// Serve these tools to AI agents over MCP (stdio).
    Mcp,
    /// Show what `pack` would use here: the last failure, your changes and the baseline.
    Status(status::StatusArgs),
    /// Create .codigest/config.toml (file filters). `cdg mark` sets a baseline.
    Init(init::InitArgs),
    /// Delete .codigest/: the config, the baseline and the cache.
    Clean(clean::CleanArgs),
}

/// Runs the parsed command and returns the process exit code.
pub fn run(cli: Cli) -> anyhow::Result<i32> {
    match cli.command {
        Command::Scan(args) => scan::run(args),
        Command::Digest(args) => digest::run(args),
        Command::Tree(args) => tree::run(args),
        Command::Mark(args) => changes::mark(args),
        Command::Diff(args) => changes::diff(args),
        Command::Semdiff(args) => changes::semdiff(args),
        Command::Cap(args) => cap::run(args),
        Command::Pack(args) => pack::run(args),
        Command::Term(args) => term::run(args),
        Command::Mcp => mcp::run(),
        Command::Status(args) => status::run(args),
        Command::Init(args) => init::run(args),
        Command::Clean(args) => clean::run(args),
    }
}
