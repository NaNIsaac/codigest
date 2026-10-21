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
