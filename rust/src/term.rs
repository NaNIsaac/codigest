//! The terminal capture store shared with the Python build (`core/termstore.py`):
//! one JSON object per line in `<data dir>/term/history.jsonl`.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use chrono::Local;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::redact::{is_denied_command, redact};
use crate::tags::{escape_attr, escape_value};
use crate::{format, paths};

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

/// Entries as Markdown: a heading per command, its details as a list, and the command with
/// its output in one console block, the way it looked in the terminal.
pub fn render_markdown(entries: &[Entry], include_context: bool) -> String {
    let total = entries.len();
    let blocks: Vec<String> = entries
        .iter()
        .enumerate()
        .map(|(offset, entry)| {
            let status = match entry.exit_code {
                Some(code) => format!(" (exit {code})"),
                None => String::new(),
            };
            let mut lines = vec![
                format!("## Command {}{status}", total - offset),
                String::new(),
            ];
            let mut detail = |name: &str, value: &str| {
                if !value.is_empty() {
                    lines.push(format!("- {name}: {value}"));
                }
            };
            detail("cwd", &entry.cwd);
            if include_context {
                detail("hostname", &entry.hostname);
                detail("shell", &entry.shell);
            }
            if entry.duration_ms > 0 {
                detail("duration", &fmt_duration(entry.duration_ms));
            }
            detail("at", &entry.started_at);
            if entry.truncated {
                detail("truncated", "middle of the output left out");
            }
            if !entry.captured {
                detail("output", "not stored (read from shell history)");
            }
            detail("note", &entry.note);
            if lines.len() > 2 {
                lines.push(String::new());
            }
            let mut console = format!("$ {}", entry.command);
            if !entry.output.is_empty() {
                console.push('\n');
                console.push_str(&entry.output);
            }
            let fence = format::fence(&console);
            lines.push(format!("{fence}console\n{console}\n{fence}"));
            lines.join("\n")
        })
        .collect();
    blocks.join("\n\n")
}

/// Entries as a JSON array, newest last, each with its `index` counting back from the latest.
pub fn render_json(entries: &[Entry], include_context: bool) -> anyhow::Result<String> {
    let total = entries.len();
    let items: Vec<serde_json::Value> = entries
        .iter()
        .enumerate()
        .map(|(offset, entry)| {
            let mut item = serde_json::json!({
                "index": total - offset,
                "command": entry.command,
                "exit_code": entry.exit_code,
                "cwd": entry.cwd,
                "started_at": entry.started_at,
                "duration_ms": entry.duration_ms,
                "output": entry.output,
                "output_available": entry.captured,
                "truncated": entry.truncated,
            });
            if include_context {
                item["hostname"] = entry.hostname.clone().into();
                item["shell"] = entry.shell.clone().into();
            }
            if !entry.note.is_empty() {
                item["note"] = entry.note.clone().into();
            }
            item
        })
        .collect();
    Ok(serde_json::to_string_pretty(&items)?)
}

const MAX_ENTRIES: usize = 200;
const TRIM_SLACK: usize = 50;
pub const MAX_OUTPUT_BYTES: usize = 64 * 1024;
const HEAD_BYTES: usize = MAX_OUTPUT_BYTES / 2;
const TAIL_BYTES: usize = MAX_OUTPUT_BYTES - HEAD_BYTES;

// CSI / OSC / single-char escapes emitted by colored CLI output.
static ANSI: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"\x1B\][^\x07\x1B]*(?:\x07|\x1B\\)", // OSC ... BEL / ST
        r"|\x1B[@-Z\\-_]",                    // Fe escapes
        r"|\x1B\[[0-?]*[ -/]*[@-~]",          // CSI
    ))
    .expect("valid pattern")
});

pub fn term_dir() -> PathBuf {
    paths::data_dir().join("term")
}

/// `datetime.now().astimezone().isoformat(timespec="seconds")`.
pub fn now_iso() -> String {
    Local::now().format("%Y-%m-%dT%H:%M:%S%:z").to_string()
}

pub fn strip_ansi(text: &str) -> String {
    ANSI.replace_all(text, "").into_owned()
}

/// Collapses progress-bar redraws: within a line, only the final `\r` segment survives.
/// Without this a single pytest/pip run can store megabytes of overwritten frames. CRLF
/// line endings (Windows) are not redraws.
pub fn collapse_carriage_returns(text: &str) -> String {
    let text = text.replace("\r\n", "\n");
    if !text.contains('\r') {
        return text;
    }
    text.split('\n')
        .map(|line| line.rsplit('\r').next().unwrap_or(line))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Python's `str.isspace`, which also counts the four ASCII information separators.
pub fn is_space(c: char) -> bool {
    c.is_whitespace() || ('\x1c'..='\x1f').contains(&c)
}

pub fn normalize_output(raw: &str) -> String {
    collapse_carriage_returns(&strip_ansi(raw))
        .trim_end_matches(is_space)
        .to_string()
}

/// UTF-8 decoding that drops what it cannot read, as Python's `errors="ignore"`: a cut
/// through a multi-byte character loses that character, not the whole half.
fn decode_ignore(bytes: &[u8]) -> String {
    bytes.utf8_chunks().map(|chunk| chunk.valid()).collect()
}

/// Output beyond 64 KB keeps its first and last 32 KB.
pub fn truncate(text: &str) -> (String, bool) {
    let bytes = text.as_bytes();
    if bytes.len() <= MAX_OUTPUT_BYTES {
        return (text.to_string(), false);
    }
    let head = decode_ignore(&bytes[..HEAD_BYTES]);
    let tail = decode_ignore(&bytes[bytes.len() - TAIL_BYTES..]);
    let elided = bytes.len() - HEAD_BYTES - TAIL_BYTES;
    (
        format!("{head}\n\n... [{elided} bytes elided by codigest] ...\n\n{tail}"),
        true,
    )
}

/// `json.dumps(..., ensure_ascii=False)` separates with ", " and ": ", which serde_json's
/// compact form does not; the store is shared with the Python build, so keep its layout.
struct PythonFormat;

impl serde_json::ser::Formatter for PythonFormat {
    fn begin_object_key<W: ?Sized + io::Write>(
        &mut self,
        w: &mut W,
        first: bool,
    ) -> io::Result<()> {
        if first { Ok(()) } else { w.write_all(b", ") }
    }

    fn begin_object_value<W: ?Sized + io::Write>(&mut self, w: &mut W) -> io::Result<()> {
        w.write_all(b": ")
    }
}

fn json_line(entry: &Entry) -> anyhow::Result<String> {
    let mut buf = Vec::new();
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, PythonFormat);
    entry.serialize(&mut ser)?;
    Ok(String::from_utf8(buf)?)
}

/// Normalizes, redacts, truncates and appends an entry to the store, then trims the ring
/// buffer. Denied commands are stored without their output.
/// Returns the entry as stored: normalized, redacted and truncated.
pub fn append_entry(entry: Entry, allow_secrets: bool) -> anyhow::Result<Entry> {
    append_entry_in(&term_dir(), entry, allow_secrets)
}

pub fn append_entry_in(dir: &Path, mut entry: Entry, allow_secrets: bool) -> anyhow::Result<Entry> {
    entry.output = normalize_output(&entry.output);
    // Recorded at capture time: exporting old records must not relabel output from
    // another machine as having run on the current host.
    if entry.captured && entry.hostname.is_empty() {
        entry.hostname = gethostname::gethostname().to_string_lossy().into_owned();
    }

    if !allow_secrets {
        if is_denied_command(&entry.command) {
            entry.output.clear();
            entry.note = "output withheld (command may expose credentials)".into();
        } else {
            entry.output = redact(&entry.output);
            entry.command = redact(&entry.command);
        }
    }

    (entry.output, entry.truncated) = truncate(&entry.output);

    fs::create_dir_all(dir)?;
    let path = dir.join("history.jsonl");
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;
    writeln!(file, "{}", json_line(&entry)?)?;
    drop(file);

    trim(&path)?;
    restrict_permissions(dir, &path);
    Ok(entry)
}

/// Rewrites the log keeping only the most recent `MAX_ENTRIES` lines.
fn trim(path: &Path) -> anyhow::Result<()> {
    let text = String::from_utf8_lossy(&fs::read(path)?).into_owned();
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    // No file lock: concurrent hook ingests can drop an entry appended between read and
    // replace; the slack makes rewrites (and that window) rare.
    if lines.len() <= MAX_ENTRIES + TRIM_SLACK {
        return Ok(());
    }
    let tmp = path.with_extension("jsonl.tmp");
    fs::write(&tmp, lines[lines.len() - MAX_ENTRIES..].concat())?;
    fs::rename(&tmp, path)?;
    Ok(())
}

/// Best-effort owner-only permissions on POSIX; a no-op on Windows.
#[cfg(unix)]
fn restrict_permissions(dir: &Path, path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o700));
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
}

#[cfg(not(unix))]
fn restrict_permissions(_dir: &Path, _path: &Path) {}

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

    #[test]
    fn strips_escapes_and_collapses_redraws() {
        assert_eq!(
            strip_ansi("\x1b[31mred\x1b[0m \x1b]0;title\x07ok"),
            "red ok"
        );
        assert_eq!(
            normalize_output("10%\r50%\r100%\r\ndone\r\n\n \t"),
            "100%\ndone"
        );
        assert_eq!(normalize_output("a\r\nb"), "a\nb");
    }

    #[test]
    fn truncation_keeps_head_and_tail_on_character_boundaries() {
        let (same, cut) = truncate("short");
        assert_eq!((same.as_str(), cut), ("short", false));

        // 3-byte characters, so both cut points fall inside one.
        let text = "가".repeat(30_000);
        let (shown, cut) = truncate(&text);
        assert!(cut);
        let marker = "\n\n... [24464 bytes elided by codigest] ...\n\n";
        let (head, tail) = shown.split_once(marker).expect("marker");
        assert_eq!(head.len(), 32_766);
        assert_eq!(tail.len(), 32_766);
        assert!(head.chars().all(|c| c == '가') && tail.chars().all(|c| c == '가'));
    }

    fn append(dir: &Path, command: &str, output: &str) {
        let entry = Entry {
            command: command.into(),
            output: output.into(),
            exit_code: Some(0),
            started_at: "2026-01-01T00:00:00+00:00".into(),
            ..Entry::default()
        };
        append_entry_in(dir, entry, false).unwrap();
    }

    #[test]
    fn appended_entries_are_normalized_and_redacted() {
        let dir = tempfile::tempdir().unwrap();
        append(
            dir.path(),
            "curl -u me:pw",
            "\x1b[1mtoken = abcdef123456\x1b[0m \n",
        );
        let entries = load_from(&dir.path().join("history.jsonl"));
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].output, "token = [REDACTED]");
        assert!(entries[0].captured && !entries[0].hostname.is_empty());
        assert!(!entries[0].truncated);
    }

    #[test]
    fn denied_commands_keep_no_output() {
        let dir = tempfile::tempdir().unwrap();
        append(dir.path(), "printenv", "SECRET=1");
        let entries = load_from(&dir.path().join("history.jsonl"));
        assert_eq!(entries[0].output, "");
        assert_eq!(
            entries[0].note,
            "output withheld (command may expose credentials)"
        );
    }

    #[test]
    fn lines_use_the_python_layout() {
        let dir = tempfile::tempdir().unwrap();
        let entry = Entry {
            command: "echo é".into(),
            hostname: "h".into(),
            started_at: "t".into(),
            ..Entry::default()
        };
        append_entry_in(dir.path(), entry, true).unwrap();
        let text = fs::read_to_string(dir.path().join("history.jsonl")).unwrap();
        assert_eq!(
            text,
            "{\"command\": \"echo é\", \"output\": \"\", \"exit_code\": null, \"cwd\": \"\", \
             \"shell\": \"\", \"hostname\": \"h\", \"started_at\": \"t\", \"duration_ms\": 0, \
             \"truncated\": false, \"captured\": true, \"note\": \"\"}\n"
        );
    }

    #[test]
    fn the_ring_buffer_trims_in_batches() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.jsonl");
        let lines = |p: &Path| fs::read_to_string(p).unwrap().lines().count();
        for i in 0..(MAX_ENTRIES + TRIM_SLACK) {
            append(dir.path(), &format!("c{i}"), "");
        }
        assert_eq!(lines(&path), MAX_ENTRIES + TRIM_SLACK);
        append(dir.path(), "last", "");
        assert_eq!(lines(&path), MAX_ENTRIES);
        let entries = load_from(&path);
        assert_eq!(entries.last().unwrap().command, "last");
        assert_eq!(entries[0].command, "c51");
    }

    #[test]
    fn timestamps_have_the_python_shape() {
        let shape = Regex::new(r"^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d[+-]\d\d:\d\d$").unwrap();
        assert!(shape.is_match(&now_iso()), "{}", now_iso());
    }
}
