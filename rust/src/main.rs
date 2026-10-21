use std::process::ExitCode;

use clap::Parser;
use codigest::commands::{self, Cli};
use codigest::style::red;

fn main() -> ExitCode {
    // Usage errors exit with 2, as they did under click.
    let cli = Cli::parse();
    match commands::run(cli) {
        Ok(code) => ExitCode::from(u8::try_from(code).unwrap_or(1)),
        Err(err) => {
            eprintln!("{} {err:#}", red("Error:"));
            ExitCode::FAILURE
        }
    }
}
