//! Prints every command of the Rust build with its options as JSON, read from clap itself
//! (hidden commands and options included), for `docs/cli_matrix.py`.
//!
//!     cargo run --quiet --example cli_options

use clap::{Command, CommandFactory};
use codigest::commands::Cli;
use serde_json::{Map, Value, json};

/// Each option's names, long names first (`--copy`, then aliases, then `-c`), and whether it is
/// hidden from `--help`.
fn options(command: &Command) -> Vec<Value> {
    command
        .get_arguments()
        .filter(|a| !a.is_positional())
        .filter_map(|arg| {
            let mut names: Vec<String> = Vec::new();
            names.extend(arg.get_long().map(|l| format!("--{l}")));
            names.extend(
                arg.get_all_aliases()
                    .unwrap_or_default()
                    .iter()
                    .map(|l| format!("--{l}")),
            );
            names.extend(arg.get_short().map(|c| format!("-{c}")));
            names.extend(
                arg.get_all_short_aliases()
                    .unwrap_or_default()
                    .iter()
                    .map(|c| format!("-{c}")),
            );
            names.retain(|n| n != "-h" && n != "--help");
            (!names.is_empty()).then(|| json!({"names": names, "hidden": arg.is_hide_set()}))
        })
        .collect()
}

fn walk(command: &Command, path: &[&str], hidden: bool, out: &mut Map<String, Value>) {
    out.insert(
        path.join(" "),
        json!({"hidden": hidden, "options": options(command)}),
    );
    for sub in command.get_subcommands().filter(|s| s.get_name() != "help") {
        let mut sub_path = path.to_vec();
        sub_path.push(sub.get_name());
        walk(sub, &sub_path, hidden || sub.is_hide_set(), out);
    }
}

fn main() {
    let mut out = Map::new();
    walk(&Cli::command(), &[], false, &mut out);
    println!(
        "{}",
        serde_json::to_string_pretty(&Value::Object(out)).expect("valid JSON")
    );
}
