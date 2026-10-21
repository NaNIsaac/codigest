//! `cap`: run a command transparently and record it for later recall.
//!
//! Everything after `cap` belongs to the wrapped command, so flags like `-n` are never
//! stolen from it. Only output options written before the command (`cdg cap -c -- pytest`)
//! are read, by hand, and they make `cap` hand over the result like `cdg term` would.

use std::io::Write;
use std::path::PathBuf;

use clap::{Args, ValueEnum};

use crate::format::Format;
use crate::output::{Output, OutputArgs};
use crate::runner::{self, RunError};
use crate::style::{dim, red, yellow};
use crate::{shellhist, term};

#[derive(Args, Debug)]
#[command(trailing_var_arg = true)]
pub struct CapArgs {
    /// Command to run. Prefix with `--` so its flags are passed through. Output options
    /// before it (-c, -s, -o FILE, --format) also hand over the result like `cdg term`.
    #[arg(value_name = "COMMAND...", num_args = 0.., allow_hyphen_values = true)]
    pub command: Vec<String>,
}

/// What `cap` does with the result once the command has run.
#[derive(Default)]
struct Delivery {
    out: OutputArgs,
    format: Format,
    requested: bool,
}

pub fn run(args: CapArgs) -> anyhow::Result<i32> {
    let (delivery, command) = match split_options(args.command) {
        Ok(split) => split,
        Err(message) => {
            eprintln!("{} {message}", red("[Error]"));
            return Ok(2);
        }
    };
    if command.is_empty() {
        eprintln!(
            "{} cdg cap -- <command>\n{}",
            yellow("Usage:"),
            dim("Example: cdg cap -- pytest -q")
        );
        return Ok(2);
    }

    let (command_text, result) = match script_block(&command) {
        Some(script) => {
            let Some(shell) = runner::which("pwsh").or_else(|| runner::which("powershell")) else {
                eprintln!(
                    "{} PowerShell is needed to run a {{ }} block",
                    red("[Error]")
                );
                return Ok(127);
            };
            let argv = [
                shell.to_string_lossy().into_owned(),
                "-NoProfile".into(),
                "-EncodedCommand".into(),
                command[1].clone(),
            ];
            (script.trim().to_string(), runner::run_argv(&argv))
        }
        None => (runner::join_command(&command), runner::run(&command)),
    };
    let captured = match result {
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
    let _ = std::io::stdout().flush();
    match term::append_entry(entry, false) {
        Ok(stored) if delivery.requested => {
            let entries = [stored];
            let content = match delivery.format {
                Format::Xml => term::render(&entries, false),
                Format::Markdown => term::render_markdown(&entries, false),
                Format::Json => term::render_json(&entries, false)?,
            };
            // The output already went by on screen, so there is nothing to summarize.
            Output::new(&delivery.out, true).deliver(&content, "1 terminal entry", |_| {})?;
        }
        Ok(_) => {}
        Err(err) => eprintln!("{}", dim(&format!("Capture not stored: {err}"))),
    }

    // The exit code is passed through whole: a Windows code such as -4058 does not fit
    // the `u8` the other commands return through `main`.
    let _ = std::io::stdout().flush();
    std::process::exit(captured.exit_code);
}

/// Reads the output options in front of the command. Parsed by hand because clap would
/// also pick them out of the wrapped command's own arguments.
fn split_options(args: Vec<String>) -> Result<(Delivery, Vec<String>), String> {
    let mut delivery = Delivery::default();
    let mut rest = args.into_iter().peekable();
    while let Some(arg) = rest.peek() {
        let (name, inline) = match arg.split_once('=') {
            Some((name, value)) if name.starts_with("--") => {
                (name.to_string(), Some(value.to_string()))
            }
            _ => (arg.clone(), None),
        };
        let takes_value = matches!(name.as_str(), "-o" | "--output" | "--format");
        if !takes_value && inline.is_some() {
            break;
        }
        match name.as_str() {
            "-c" | "--copy" => delivery.out.copy = true,
            "--no-copy" => delivery.out.no_copy = true,
            "-s" | "--stdout" => delivery.out.stdout = true,
            // `cdg cap -s -- pytest`: clap keeps a `--` that comes after the first value.
            "--" => {
                rest.next();
                break;
            }
            "-o" | "--output" | "--format" => {
                rest.next();
                let value = inline
                    .or_else(|| rest.next())
                    .ok_or_else(|| format!("{name} needs a value"))?;
                if name == "--format" {
                    delivery.format = Format::from_str(&value, true)
                        .map_err(|_| format!("unknown format '{value}' (xml, markdown or json)"))?;
                } else {
                    delivery.out.output = Some(PathBuf::from(value));
                }
                delivery.requested = true;
                continue;
            }
            _ => break,
        }
        delivery.requested = true;
        rest.next();
    }
    Ok((delivery, rest.collect()))
}

/// PowerShell hands a `{ ... }` argument to a program as `-encodedCommand <base64 of
/// UTF-16LE>` followed by `-inputFormat xml -outputFormat text`, the form `pwsh { ... }`
/// receives. Returns the script text.
fn script_block(args: &[String]) -> Option<String> {
    let [flag, encoded, ..] = args else {
        return None;
    };
    if !flag.eq_ignore_ascii_case("-encodedCommand") {
        return None;
    }
    let bytes = base64_decode(encoded)?;
    let units: Vec<u16> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&pair| u16::from_le_bytes(pair))
        .collect();
    String::from_utf16(&units).ok()
}

fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let (mut acc, mut bits) = (0u32, 0u32);
    for c in text.bytes().take_while(|&c| c != b'=') {
        let value = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        };
        acc = acc << 6 | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn a_block_from_powershell_is_decoded() {
        // What PowerShell passes for `{ ls }`.
        let block = args(&[
            "-encodedCommand",
            "IABsAHMAIAA=",
            "-inputFormat",
            "xml",
            "-outputFormat",
            "text",
        ]);
        assert_eq!(script_block(&block).as_deref(), Some(" ls "));
        assert_eq!(script_block(&args(&["pytest", "-q"])), None);
    }

    #[test]
    fn options_before_the_command_are_read_and_the_rest_is_left_alone() {
        let (delivery, command) =
            split_options(args(&["-c", "--format=markdown", "pytest", "-s", "-c"])).unwrap();
        assert!(delivery.requested && delivery.out.copy && !delivery.out.stdout);
        assert_eq!(delivery.format, Format::Markdown);
        assert_eq!(command, args(&["pytest", "-s", "-c"]));

        let (delivery, command) = split_options(args(&["pytest", "-q"])).unwrap();
        assert!(!delivery.requested);
        assert_eq!(command, args(&["pytest", "-q"]));

        assert!(split_options(args(&["--format", "yaml", "ls"])).is_err());
    }
}
