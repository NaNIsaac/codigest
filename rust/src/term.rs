//! The terminal capture store shared with the Python build (`core/termstore.py`):
//! one JSON object per line in `<data dir>/term/history.jsonl`.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::paths;
use crate::tags::{escape_attr, escape_value};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Entry {
    pub command: String,
    pub output: String,
    pub exit_code: Option<i64>,
    pub cwd: String,
    pub shell: String,
    pub hostname: String,
    pub started_at: String,
    pub duration_ms: u64,
    pub truncated: bool,
    /// False when the entry was rebuilt from shell history and has no output.
    pub captured: bool,
    pub note: String,
}

impl Default for Entry {
    fn default() -> Self {
        Self {
            command: String::new(),
            output: String::new(),
            exit_code: None,
            cwd: String::new(),
            shell: String::new(),
            hostname: String::new(),
            started_at: String::new(),
            duration_ms: 0,
            truncated: false,
            captured: true,
            note: String::new(),
        }
    }
}

impl Entry {
    pub fn failed(&self) -> bool {
        !matches!(self.exit_code, None | Some(0))
    }
}

/// All stored entries, oldest first. Lines that are not an entry are skipped, and unknown
/// keys are ignored, so files written by other versions still load.
pub fn load_entries() -> Vec<Entry> {
    load_from(&paths::history_path())
}

pub fn load_from(path: &Path) -> Vec<Entry> {
    let Ok(bytes) = fs::read(path) else {
        return Vec::new();
    };
    let text = String::from_utf8_lossy(&bytes);
    let mut entries: Vec<Entry> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();
    // Shell-hook ingests run in background processes and may land out of order.
    entries.sort_by(|a, b| a.started_at.cmp(&b.started_at));
    entries
}

/// Keeps entries that exited non-zero (`failed`) and/or ran inside `under` or below it.
pub fn select(entries: Vec<Entry>, failed: bool, under: Option<&Path>) -> Vec<Entry> {
    let under = under.map(normcase);
    entries
        .into_iter()
        .filter(|e| !failed || e.failed())
        .filter(|e| match &under {
            Some(base) => !e.cwd.is_empty() && inside(&normcase(Path::new(&e.cwd)), base),
            None => true,
        })
        .collect()
}

fn inside(path: &str, base: &str) -> bool {
    let sep = std::path::MAIN_SEPARATOR;
    path == base || path.starts_with(&format!("{}{sep}", base.trim_end_matches(sep)))
}

/// `os.path.normcase(os.path.abspath(p))`: absolute, and on Windows lower-case with
/// backslashes, so `C:/Repo` and `c:\repo` compare equal.
fn normcase(path: &Path) -> String {
    let absolute: PathBuf = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let text = absolute.to_string_lossy().into_owned();
    if cfg!(windows) {
        text.replace('/', "\\").to_lowercase()
    } else {
        text
    }
}

fn fmt_duration(ms: u64) -> String {
    if ms < 1000 {
        format!("{ms}ms")
    } else {
        format!("{:.1}s", ms as f64 / 1000.0)
    }
}

/// Entries as one XML block. `index` counts back from the most recent: index="1" is the
/// last command. Hostname and shell are opt-in, since names can expose organization details.
pub fn render(entries: &[Entry], include_context: bool) -> String {
    if entries.is_empty() {
        return "<terminal_history />".to_string();
    }
    let total = entries.len();
    let blocks: Vec<String> = entries
        .iter()
        .enumerate()
        .map(|(offset, entry)| {
            let mut attrs = vec![
                format!("index=\"{}\"", total - offset),
                format!("command=\"{}\"", escape_attr(&entry.command)),
            ];
            if let Some(code) = entry.exit_code {
                attrs.push(format!("exit_code=\"{code}\""));
            }
            if !entry.cwd.is_empty() {
                attrs.push(format!("cwd=\"{}\"", escape_attr(&entry.cwd)));
            }
            if include_context && !entry.hostname.is_empty() {
                attrs.push(format!("hostname=\"{}\"", escape_attr(&entry.hostname)));
            }
            if include_context && !entry.shell.is_empty() {
                attrs.push(format!("shell=\"{}\"", escape_attr(&entry.shell)));
            }
            if entry.duration_ms > 0 {
                attrs.push(format!("duration=\"{}\"", fmt_duration(entry.duration_ms)));
            }
            if !entry.started_at.is_empty() {
                attrs.push(format!("at=\"{}\"", entry.started_at));
            }
            if entry.truncated {
                attrs.push("truncated=\"true\"".to_string());
            }
            if !entry.captured {
                attrs.push("output_available=\"false\"".to_string());
            }
            let mut body = Vec::new();
            if !entry.note.is_empty() {
                body.push(format!("<note>{}</note>", escape_value(&entry.note)));
            }
            if entry.captured || !entry.output.is_empty() {
                body.push(format!(
                    "<output>\n{}\n</output>",
                    escape_value(&entry.output)
                ));
            }
            format!(
                "<command {}>\n{}\n</command>",
                attrs.join(" "),
                body.join("\n")
            )
        })
        .collect();
    format!(
        "<terminal_history>\n{}\n</terminal_history>",
        blocks.join("\n")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(command: &str, exit_code: Option<i64>, at: &str) -> Entry {
        Entry {
            command: command.into(),
            exit_code,
            started_at: at.into(),
            ..Entry::default()
        }
    }

    #[test]
    fn loads_old_new_and_broken_lines_in_time_order() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.jsonl");
        fs::write(
            &path,
            concat!(
                r#"{"command": "b", "started_at": "2026-01-02", "extra": 1}"#,
                "\n",
                "not json\n\n",
                r#"{"command": "a", "started_at": "2026-01-01", "exit_code": null}"#,
                "\n",
            ),
        )
        .unwrap();
        let entries = load_from(&path);
        assert_eq!(
            entries
                .iter()
                .map(|e| e.command.as_str())
                .collect::<Vec<_>>(),
            ["a", "b"]
        );
        assert!(entries[0].captured);
    }

    #[test]
    fn selects_failures() {
        let all = vec![
            entry("ok", Some(0), "1"),
            entry("bad", Some(2), "2"),
            entry("none", None, "3"),
        ];
        let failed = select(all, true, None);
        assert_eq!(failed.len(), 1);
        assert_eq!(failed[0].command, "bad");
    }

    #[test]
    fn renders_like_the_python_build() {
        let e = Entry {
            command: r#"pytest -k "a""#.into(),
            output: "E  1 -> 2".into(),
            exit_code: Some(1),
            cwd: "/p".into(),
            duration_ms: 1500,
            started_at: "2026-10-17T01:58:14+09:00".into(),
            ..Entry::default()
        };
        assert_eq!(
            render(&[e], false),
            "<terminal_history>\n<command index=\"1\" command=\"pytest -k &quot;a&quot;\" exit_code=\"1\" \
             cwd=\"/p\" duration=\"1.5s\" at=\"2026-10-17T01:58:14+09:00\">\n<output>\nE  1 -> 2\n</output>\n\
             </command>\n</terminal_history>"
        );
    }
}
