//! `--preset NAME`: option sets kept in TOML, expanded into the command line before it is
//! parsed. Options given on the command line come after the preset's, and the later one
//! wins, so a preset only fills in what was not said.
//!
//! ```toml
//! [presets.term.debug]
//! format = "markdown"
//! count = 3
//! failed = true
//! ```
//!
//! Presets are read from the user configuration file and then from the project's
//! `.codigest/config.toml`; a project preset replaces a user preset of the same name.

use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use toml::Value;

/// `%APPDATA%\codigest\config.toml` on Windows, else `$XDG_CONFIG_HOME/codigest/config.toml`
/// or `~/.config/codigest/config.toml`.
pub fn user_config() -> PathBuf {
    let base = |var: &str| {
        env::var_os(var)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    let dir = if cfg!(windows) {
        base("APPDATA").unwrap_or_else(|| crate::paths::home().join("AppData").join("Roaming"))
    } else {
        base("XDG_CONFIG_HOME").unwrap_or_else(|| crate::paths::home().join(".config"))
    };
    dir.join("codigest").join("config.toml")
}

/// The nearest `.codigest/config.toml` from `start` upwards, stopping at a Git root.
fn project_config(start: &Path) -> Option<PathBuf> {
    for dir in start.ancestors() {
        let candidate = dir.join(".codigest").join("config.toml");
        if candidate.is_file() {
            return Some(candidate);
        }
        if dir.join(".git").exists() {
            return None;
        }
    }
    None
}

fn preset_from(file: &Path, command: &str, name: &str) -> Result<Option<toml::Table>> {
    let Ok(text) = fs::read_to_string(file) else {
        return Ok(None);
    };
    let doc: toml::Table = text
        .parse()
        .with_context(|| format!("reading {}", file.display()))?;
    Ok(doc
        .get("presets")
        .and_then(|p| p.get(command))
        .and_then(|c| c.get(name))
        .and_then(Value::as_table)
        .cloned())
}

fn to_args(table: &toml::Table, origin: &str) -> Result<Vec<OsString>> {
    let mut args = Vec::new();
    for (key, value) in table {
        let flag = format!("--{}", key.replace('_', "-"));
        match value {
            Value::Boolean(true) => args.push(flag.into()),
            Value::Boolean(false) => {}
            Value::String(s) => args.extend([flag.into(), s.into()]),
            Value::Integer(i) => args.extend([flag.into(), i.to_string().into()]),
            Value::Float(f) => args.extend([flag.into(), f.to_string().into()]),
            Value::Array(items) => {
                let parts: Vec<String> = items
                    .iter()
                    .map(|v| v.as_str().map_or_else(|| v.to_string(), String::from))
                    .collect();
                args.extend([flag.into(), parts.join(",").into()]);
            }
            other => bail!("{origin}: option {key} cannot take {}", other.type_str()),
        }
    }
    Ok(args)
}

/// `argv` with `--preset NAME` replaced by the preset's options, placed right after the
/// command name so that anything typed after it overrides them.
pub fn expand(argv: Vec<OsString>) -> Result<Vec<OsString>> {
    let strings: Vec<String> = argv
        .iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    let Some(at) = strings
        .iter()
        .position(|a| a == "--preset" || a.starts_with("--preset="))
    else {
        return Ok(argv);
    };
    let (name, width) = match strings[at].strip_prefix("--preset=") {
        Some(name) => (name.to_string(), 1),
        None => match strings.get(at + 1) {
            Some(name) => (name.clone(), 2),
            None => bail!("--preset needs a name"),
        },
    };
    // The command is the first argument after the program that is not an option.
    let Some(command_at) = strings
        .iter()
        .skip(1)
        .position(|a| !a.starts_with('-'))
        .map(|i| i + 1)
    else {
        bail!("--preset needs a command");
    };
    let command = strings[command_at].clone();

    let user = user_config();
    let project = env::current_dir().ok().and_then(|cwd| project_config(&cwd));
    let mut found = None;
    for file in [Some(user.clone()), project].into_iter().flatten() {
        if let Some(table) = preset_from(&file, &command, &name)? {
            found = Some((table, file));
        }
    }
    let Some((table, file)) = found else {
        bail!(
            "no preset {name:?} for {command}; define [presets.{command}.{name}] in {} or .codigest/config.toml",
            user.display()
        );
    };
    let inserted = to_args(&table, &format!("preset {name} in {}", file.display()))?;

    let mut out: Vec<OsString> = Vec::with_capacity(argv.len() + inserted.len());
    for (i, arg) in argv.into_iter().enumerate() {
        if i >= at && i < at + width {
            continue;
        }
        out.push(arg);
        if i == command_at {
            out.extend(inserted.iter().cloned());
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_become_flags() {
        let table: toml::Table =
            "format = \"markdown\"\nfields = [\"command\", \"output\"]\ncount = 3\nfailed = true\nhere = false"
                .parse()
                .unwrap();
        let args: Vec<String> = to_args(&table, "t")
            .unwrap()
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        // Keys come out sorted; their order does not matter, as each is a different option.
        assert_eq!(
            args,
            [
                "--count",
                "3",
                "--failed",
                "--fields",
                "command,output",
                "--format",
                "markdown"
            ]
        );
    }
}
