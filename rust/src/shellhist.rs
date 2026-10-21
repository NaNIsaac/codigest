//! Native shell history, the fallback when nothing was captured (`core/shellhist.py`).
//! Commands come back without output; no shell stores that.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::time::SystemTime;

use regex::Regex;

use crate::paths::home;
use crate::term::Entry;

static SELF: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*(cdg|codigest|cdg-rs)(\.exe)?\s+term\b").expect("valid pattern")
});
static ZSH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^:\s*\d+:\d+;(.*)$").expect("valid pattern"));
static FISH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^-\s+cmd:\s*(.*)$").expect("valid pattern"));

type Parser = fn(&str) -> Vec<String>;

fn read_lines(text: &str) -> impl Iterator<Item = &str> {
    text.lines()
}

/// PSReadLine and bash: one command per line (bash may carry `#<timestamp>` lines).
fn parse_plain(text: &str) -> Vec<String> {
    read_lines(text)
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .map(String::from)
        .collect()
}

/// zsh extended history: `: <start>:<elapsed>;<command>`.
fn parse_zsh(text: &str) -> Vec<String> {
    read_lines(text)
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            ZSH.captures(l)
                .map_or(l, |c| c.get(1).expect("group").as_str())
                .to_string()
        })
        .collect()
}

/// fish history: YAML-ish `- cmd: <command>` records.
fn parse_fish(text: &str) -> Vec<String> {
    read_lines(text)
        .filter_map(|l| FISH.captures(l).map(|c| c[1].to_string()))
        .collect()
}

fn candidates() -> Vec<(&'static str, Option<PathBuf>, Parser)> {
    let home = home();
    let psreadline = env::var_os("APPDATA")
        .filter(|v| !v.is_empty())
        .map(|appdata| {
            PathBuf::from(appdata)
                .join(r"Microsoft\Windows\PowerShell\PSReadLine\ConsoleHost_history.txt")
        });
    vec![
        ("powershell", psreadline, parse_plain as Parser),
        ("zsh", Some(home.join(".zsh_history")), parse_zsh),
        ("bash", Some(home.join(".bash_history")), parse_plain),
        (
            "fish",
            Some(home.join(".local/share/fish/fish_history")),
            parse_fish,
        ),
    ]
}

/// Best-effort name of the shell that most likely launched this process.
pub fn detect_shell() -> String {
    if let Some(shell) = env::var_os("SHELL").filter(|s| !s.is_empty()) {
        return Path::new(&shell)
            .file_stem()
            .map_or_else(String::new, |s| s.to_string_lossy().into_owned());
    }
    if env::var_os("PSModulePath").is_some() {
        return "powershell".into();
    }
    if cfg!(windows) {
        "cmd".into()
    } else {
        "unknown".into()
    }
}

/// The most recent `count` commands of the most recently written history file, oldest first,
/// and the shell it belongs to.
pub fn read_recent(count: usize) -> (Vec<Entry>, String) {
    let mut best: Option<(SystemTime, &str, Vec<String>)> = None;
    for (name, path, parser) in candidates() {
        let Some(path) = path else { continue };
        let (Ok(bytes), Ok(meta)) = (fs::read(&path), fs::metadata(&path)) else {
            continue;
        };
        let commands = parser(&String::from_utf8_lossy(&bytes));
        let Ok(mtime) = meta.modified() else { continue };
        if commands.is_empty() {
            continue;
        }
        if best.as_ref().is_none_or(|(t, _, _)| mtime > *t) {
            best = Some((mtime, name, commands));
        }
    }
    let Some((_, shell, mut commands)) = best else {
        return (Vec::new(), detect_shell());
    };
    // The history file already holds the `cdg term` call that is reading it.
    while commands.last().is_some_and(|c| SELF.is_match(c)) {
        commands.pop();
    }
    let start = if count > 0 {
        commands.len().saturating_sub(count)
    } else {
        0
    };
    let entries = commands[start..]
        .iter()
        .map(|cmd| Entry {
            command: cmd.clone(),
            shell: shell.to_string(),
            captured: false,
            note: "output not captured -- command was not run under `cdg cap`".into(),
            ..Entry::default()
        })
        .collect();
    (entries, shell.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsers_read_each_format() {
        assert_eq!(parse_plain("#1700\nls -la\n\ncd x\n"), ["ls -la", "cd x"]);
        assert_eq!(
            parse_zsh(": 1700:0;git status\nplain\n"),
            ["git status", "plain"]
        );
        assert_eq!(
            parse_fish("- cmd: make\n  when: 1\n- cmd: ls\n"),
            ["make", "ls"]
        );
    }
}
