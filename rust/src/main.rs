use std::env;
use std::process::ExitCode;

use codigest::commands;
use codigest::presets;
use codigest::style::red;

fn main() -> ExitCode {
    let argv = match presets::expand(env::args_os().collect()) {
        Ok(argv) => argv,
        Err(err) => {
            eprintln!("{} {err:#}", red("Error:"));
            return ExitCode::from(2);
        }
    };
    // Usage errors exit with 2, as they did under click.
    let cli = commands::parse(argv);
    match commands::run(cli) {
        Ok(code) => ExitCode::from(u8::try_from(code).unwrap_or(1)),
        Err(err) => {
            eprintln!("{} {err:#}", red("Error:"));
            ExitCode::FAILURE
        }
    }
}
