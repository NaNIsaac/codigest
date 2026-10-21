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
pub mod shell_help;
pub mod status;
pub mod term;
pub mod tree;

use std::ffi::OsString;

use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};

/// Parses `argv` with a repeated option replacing the earlier one, so options typed after
/// a preset's override it instead of being rejected as given twice.
pub fn parse(argv: Vec<OsString>) -> Cli {
    let mut cmd = Cli::command().args_override_self(true);
    let names: Vec<String> = cmd
        .get_subcommands()
        .map(|s| s.get_name().to_string())
        .collect();
    for name in names {
        cmd = cmd.mut_subcommand(name, |sub| sub.args_override_self(true));
    }
    let matches = cmd.get_matches_from(argv);
    Cli::from_arg_matches(&matches).unwrap_or_else(|err| err.exit())
}

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
    /// Outline source files: numbered signatures and doc comments, no bodies.
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
    /// Let PowerShell Get-Help find codigest without enabling capture.
    #[command(name = "shell-help", subcommand_required = true)]
    ShellHelp(shell_help::ShellHelpArgs),
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
        Command::ShellHelp(args) => shell_help::run(args),
    }
}
