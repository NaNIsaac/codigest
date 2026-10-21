//! Where Codigest keeps machine-wide data. Resolved by hand rather than through a
//! platform-directories crate so that the Python and Rust builds share one store.

use std::env;
use std::path::PathBuf;

/// `CODIGEST_HOME`, then `%LOCALAPPDATA%\codigest` on Windows, then
/// `$XDG_DATA_HOME/codigest`, then `~/.local/share/codigest`.
pub fn data_dir() -> PathBuf {
    if let Some(dir) = non_empty("CODIGEST_HOME") {
        return expand_home(&dir);
    }
    if cfg!(windows) {
        let base = non_empty("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| home().join("AppData").join("Local"));
        return base.join("codigest");
    }
    if let Some(base) = non_empty("XDG_DATA_HOME") {
        return PathBuf::from(base).join("codigest");
    }
    home().join(".local").join("share").join("codigest")
}

pub fn history_path() -> PathBuf {
    data_dir().join("term").join("history.jsonl")
}

/// The order of Python's `sorted()` on `Path` objects: component by component, and on
/// Windows without regard to case. Comparing whole strings would put `a-b/x` before `a/x`.
pub fn cmp_paths(a: &std::path::Path, b: &std::path::Path) -> std::cmp::Ordering {
    let key = |p: &std::path::Path| -> Vec<String> {
        p.components()
            .map(|c| {
                let s = c.as_os_str().to_string_lossy().into_owned();
                if cfg!(windows) { s.to_lowercase() } else { s }
            })
            .collect()
    };
    key(a).cmp(&key(b))
}

/// Drops the `\\?\` prefix `canonicalize` adds on Windows, which Python never shows.
pub fn strip_verbatim(path: PathBuf) -> PathBuf {
    let text = path.to_string_lossy();
    match text.strip_prefix(r"\\?\UNC\") {
        Some(rest) => PathBuf::from(format!(r"\\{rest}")),
        None => text
            .strip_prefix(r"\\?\")
            .map_or_else(|| path.clone(), PathBuf::from),
    }
}

fn non_empty(name: &str) -> Option<String> {
    env::var(name).ok().filter(|v| !v.is_empty())
}

pub fn home() -> PathBuf {
    env::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

fn expand_home(path: &str) -> PathBuf {
    match path.strip_prefix('~') {
        Some(rest) => home().join(rest.trim_start_matches(['/', '\\'])),
        None => PathBuf::from(path),
    }
}
