//! `term ingest` (hidden): store one entry for the shell hook, which runs it in a
//! background process. PowerShell sends JSON on stdin; bash and zsh send options plus a
//! capture file holding `command NUL output`.
//!
//! The command travels in the file, not argv, so no shell or argument parser touches it.

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use chrono::{Local, TimeZone};
use clap::Args;

use crate::term::{self, Entry};

#[derive(Args, Debug)]
pub struct IngestArgs {
    /// Command, NUL, output; deleted afterwards
    #[arg(long)]
    pub file: Option<PathBuf>,
    #[arg(long, allow_hyphen_values = true)]
    pub exit_code: Option<i64>,
    #[arg(long, default_value = "")]
    pub cwd: String,
    #[arg(long, default_value = "")]
    pub shell: String,
    /// Epoch seconds
    #[arg(long, default_value = "")]
    pub start: String,
    /// Epoch seconds
    #[arg(long, default_value = "")]
    pub end: String,
}

pub fn run(args: IngestArgs) -> anyhow::Result<i32> {
    let result = ingest(&args);
    if let Some(file) = &args.file {
        let _ = fs::remove_file(file);
    }
    match result {
        Ok(()) => Ok(0),
        Err(err) => {
            // The hook discards our stderr; leave a trail `cdg term` can surface.
            let _ = log_error(&err);
            Ok(1)
        }
    }
}

fn ingest(args: &IngestArgs) -> anyhow::Result<()> {
    let entry = match &args.file {
        None => from_json()?,
        Some(file) => from_capture_file(file, args)?,
    };
    term::append_entry(entry, false)
}

fn from_json() -> anyhow::Result<Entry> {
    let mut bytes = Vec::new();
    std::io::stdin().lock().read_to_end(&mut bytes)?;
    let text = std::str::from_utf8(&bytes).context("stdin is not UTF-8")?;
    // PowerShell writes a byte-order mark when asked for UTF-8.
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let value: serde_json::Value = serde_json::from_str(text)?;
    if value.get("command").is_none() {
        bail!("missing required field: command");
    }
    Ok(serde_json::from_value(value)?)
}

fn from_capture_file(file: &Path, args: &IngestArgs) -> anyhow::Result<Entry> {
    let bytes = fs::read(file)?;
    let (command, output) = match bytes.iter().position(|&b| b == 0) {
        Some(at) => (&bytes[..at], &bytes[at + 1..]),
        None => (&bytes[..], &bytes[..0]),
    };
    // Locales with a decimal comma format $EPOCHREALTIME as 12,5.
    let seconds =
        |text: &str| -> anyhow::Result<f64> { Ok(text.replace(',', ".").parse::<f64>()?) };
    let or = |first: &str, second: &str, fallback: &'static str| {
        [first, second, fallback]
            .into_iter()
            .find(|s| !s.is_empty())
            .unwrap_or(fallback)
            .to_string()
    };
    let t0 = seconds(&or(&args.start, "", "0"))?;
    let t1 = seconds(&or(&args.end, &args.start, "0"))?;

    Ok(Entry {
        command: String::from_utf8_lossy(command).into_owned(),
        output: String::from_utf8_lossy(output).into_owned(),
        exit_code: args.exit_code,
        // Git Bash passes D:/x
        cwd: if args.cwd.is_empty() {
            String::new()
        } else {
            normalize_path(&args.cwd, cfg!(windows))
        },
        shell: args.shell.clone(),
        started_at: local_iso(t0)?,
        duration_ms: ((t1 - t0) * 1000.0).round_ties_even().max(0.0) as u64,
        ..Entry::default()
    })
}

/// `datetime.fromtimestamp(t).astimezone().isoformat(timespec="seconds")`.
fn local_iso(epoch: f64) -> anyhow::Result<String> {
    // Python rounds to the microsecond first, which can carry into the next second.
    let mut whole = epoch.trunc();
    let micros = (epoch.fract() * 1e6).round_ties_even();
    if micros >= 1e6 {
        whole += 1.0;
    } else if micros < 0.0 {
        whole -= 1.0;
    }
    let moment = Local
        .timestamp_opt(whole as i64, 0)
        .single()
        .context("timestamp out of range")?;
    Ok(moment.format("%Y-%m-%dT%H:%M:%S%:z").to_string())
}

/// `str(pathlib.Path(p))`: separators unified, repeated and trailing ones dropped, `.`
/// components removed. `..` is left alone, as pathlib does.
pub fn normalize_path(path: &str, windows: bool) -> String {
    let unified;
    let (prefix, rest) = if windows {
        unified = path.replace('/', "\\");
        split_windows_root(&unified)
    } else {
        split_posix_root(path)
    };
    let sep = if windows { '\\' } else { '/' };
    let parts: Vec<&str> = rest
        .split(sep)
        .filter(|p| !p.is_empty() && *p != ".")
        .collect();
    if prefix.is_empty() && parts.is_empty() {
        return ".".to_string();
    }
    format!("{prefix}{}", parts.join(&sep.to_string()))
}

/// The drive and root of a Windows path (already using backslashes), and the remainder.
fn split_windows_root(path: &str) -> (String, &str) {
    let bytes = path.as_bytes();
    if bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
        let rooted = bytes.get(2) == Some(&b'\\');
        let end = if rooted { 3 } else { 2 };
        return (path[..end].to_string(), &path[end..]);
    }
    if let Some(unc) = path.strip_prefix("\\\\") {
        let mut parts = unc.splitn(3, '\\');
        if let (Some(server), Some(share)) = (parts.next(), parts.next())
            && !server.is_empty()
            && !share.is_empty()
        {
            let rest = parts.next().unwrap_or("");
            return (format!("\\\\{server}\\{share}\\"), rest);
        }
    }
    if path.starts_with('\\') {
        return ("\\".to_string(), path.trim_start_matches('\\'));
    }
    (String::new(), path)
}

fn split_posix_root(path: &str) -> (String, &str) {
    let slashes = path.len() - path.trim_start_matches('/').len();
    // POSIX leaves exactly two leading slashes alone, and no more or fewer.
    let prefix = match slashes {
        0 => "",
        2 => "//",
        _ => "/",
    };
    (prefix.to_string(), path.trim_start_matches('/'))
}

fn log_error(err: &anyhow::Error) -> std::io::Result<()> {
    let dir = term::term_dir();
    fs::create_dir_all(&dir)?;
    let mut log = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("ingest-errors.log"))?;
    write!(log, "--- {}\n{err:?}\n", term::now_iso())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_like_pathlib() {
        assert_eq!(normalize_path("D:/x//y/./z/", true), "D:\\x\\y\\z");
        assert_eq!(normalize_path("D:/", true), "D:\\");
        assert_eq!(normalize_path("d:x", true), "d:x");
        assert_eq!(
            normalize_path("//host/share/a/", true),
            "\\\\host\\share\\a"
        );
        assert_eq!(normalize_path("/a//b/./c/", false), "/a/b/c");
        assert_eq!(normalize_path("//a", false), "//a");
        assert_eq!(normalize_path("///a", false), "/a");
        assert_eq!(normalize_path("a/../b/", false), "a/../b");
        assert_eq!(normalize_path("./", false), ".");
    }

    #[test]
    fn epoch_seconds_become_local_iso_times() {
        let shape = regex::Regex::new(r"^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d[+-]\d\d:\d\d$").unwrap();
        assert!(shape.is_match(&local_iso(1_700_000_000.25).unwrap()));
        // 0.9999996 s rounds up to the next whole second, as Python does.
        assert_eq!(
            local_iso(1_700_000_000.999_999_6).unwrap(),
            local_iso(1_700_000_001.0).unwrap()
        );
    }
}
