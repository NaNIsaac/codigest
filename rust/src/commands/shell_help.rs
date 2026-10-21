//! `shell-help install` / `uninstall`: optional, reversible PowerShell comment-based help.
//!
//! The script is the one the Python build ships, embedded here and written to the data
//! directory at install time, because a profile has to source a file that exists on disk.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use clap::{Args, Subcommand};

use crate::commands::hook::powershell_profiles;
use crate::paths;
use crate::style::{red, yellow};

const BEGIN: &str = "# >>> codigest PowerShell help >>>";
const END: &str = "# <<< codigest PowerShell help <<<";
const HELP_PS1: &str = include_str!("../../../src/codigest/shell/help.ps1");

#[derive(Args, Debug)]
pub struct ShellHelpArgs {
    #[command(subcommand)]
    pub action: ShellHelpAction,
}

#[derive(Subcommand, Debug)]
pub enum ShellHelpAction {
    /// Register PowerShell help for codigest (opt-in).
    Install,
    /// Remove only the Codigest PowerShell help integration.
    Uninstall,
}

pub fn run(args: ShellHelpArgs) -> anyhow::Result<i32> {
    configure(matches!(args.action, ShellHelpAction::Install))
}

/// Remove only the managed help block, preserving everything else.
pub fn without_block(text: &str) -> String {
    let Some(start) = text.find(BEGIN) else {
        return text.to_owned();
    };
    let Some(end) = text[start + BEGIN.len()..]
        .find(END)
        .map(|at| at + start + BEGIN.len())
    else {
        return text.to_owned();
    };
    let rest = format!("{}{}", &text[..start], &text[end + END.len()..]);
    format!("{}\n", rest.trim_matches('\n'))
}

/// The profile text with the help block added (replacing an older one).
pub fn with_block(text: &str, source: &str) -> String {
    let updated = without_block(text);
    let lead = if updated.trim_matches(crate::pystr::is_space).is_empty() {
        String::new()
    } else {
        format!("{}\n\n", updated.trim_end_matches('\n'))
    };
    format!("{lead}{BEGIN}\n{source}\n{END}\n")
}

fn script_path() -> PathBuf {
    let dir = paths::data_dir().join("shell");
    std::path::absolute(&dir).unwrap_or(dir).join("help.ps1")
}

fn source_line(script: &Path) -> String {
    let path = script.to_string_lossy().replace('\'', "''");
    format!("if (Test-Path -LiteralPath '{path}') {{ . '{path}' }}")
}

fn configure(installing: bool) -> anyhow::Result<i32> {
    let profiles = powershell_profiles();
    if profiles.is_empty() {
        println!("{}", red("PowerShell not found. Install PowerShell first."));
        return Ok(1);
    }

    let script = script_path();
    if installing {
        if let Some(parent) = script.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&script, HELP_PS1)?;
    }
    let source = source_line(&script);

    let mut seen = HashSet::new();
    for target in profiles.into_iter().filter(|p| seen.insert(p.clone())) {
        let original = if target.exists() {
            let bytes = fs::read(&target)?;
            match String::from_utf8(bytes) {
                Ok(text) => text.strip_prefix('\u{feff}').unwrap_or(&text).to_owned(),
                Err(_) => {
                    println!(
                        "{} {}",
                        yellow("Skipped non-UTF-8 profile:"),
                        target.display()
                    );
                    continue;
                }
            }
        } else {
            String::new()
        };
        let updated = if installing {
            with_block(&original, &source)
        } else {
            let updated = without_block(&original);
            if updated == original {
                continue;
            }
            updated
        };
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        // BOM so Windows PowerShell 5.1 reads non-ASCII paths correctly.
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice(updated.as_bytes());
        fs::write(&target, bytes)?;
        println!(
            "{} PowerShell help in {}",
            if installing { "Installed" } else { "Removed" },
            target.display()
        );
    }
    println!("Open a new PowerShell window to apply the change.");
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = "if (Test-Path -LiteralPath 'x') { . 'x' }";

    #[test]
    fn the_block_is_added_once_and_removed_cleanly() {
        let once = with_block("Set-Alias ll Get-ChildItem\n", SOURCE);
        assert_eq!(
            once,
            format!("Set-Alias ll Get-ChildItem\n\n{BEGIN}\n{SOURCE}\n{END}\n")
        );
        assert_eq!(with_block(&once, SOURCE), once);
        assert_eq!(without_block(&once), "Set-Alias ll Get-ChildItem\n");
        assert_eq!(without_block("no markers"), "no markers");
    }

    #[test]
    fn a_profile_with_only_the_block_is_left_as_one_newline() {
        assert_eq!(without_block(&with_block("", SOURCE)), "\n");
    }

    #[test]
    fn an_end_marker_before_the_begin_marker_is_not_a_block() {
        let text = format!("{END}\n{BEGIN}\nx\n");
        assert_eq!(without_block(&text), text);
    }

    #[test]
    fn quotes_in_the_script_path_are_doubled() {
        assert_eq!(
            source_line(Path::new("/it's/help.ps1")),
            "if (Test-Path -LiteralPath '/it''s/help.ps1') { . '/it''s/help.ps1' }"
        );
    }

    #[test]
    fn the_embedded_script_does_not_override_help_or_capture() {
        assert!(HELP_PS1.contains(".SYNOPSIS"));
        assert!(HELP_PS1.contains("function global:codigest"));
        assert!(!HELP_PS1.contains("function global:help"));
        assert!(!HELP_PS1.contains("PSConsoleHostReadLine"));
    }
}
