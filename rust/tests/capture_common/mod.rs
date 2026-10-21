//! Helpers shared by the integration tests. Not every test file uses every helper.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::{Command as Process, Output};

use assert_cmd::Command;
use regex::Regex;

/// `cdg` with an isolated data directory and no colour or width surprises.
pub fn rust(home: &Path) -> Command {
    let mut cmd = Command::cargo_bin("cdg").expect("cdg is built");
    cmd.env("CODIGEST_HOME", home)
        .env("NO_COLOR", "1")
        .env("COLUMNS", "1000")
        .env_remove("CODIGEST_HOOK")
        .env_remove("CODIGEST_SHELL");
    cmd
}

pub fn rust_exe() -> PathBuf {
    assert_cmd::cargo::cargo_bin("cdg")
}

/// The Python reference. `CODIGEST_PY` names a Python that can import `codigest` (the
/// repository's `src` is put on its path, so the checkout is what gets compared);
/// without it the installed `cdg` on PATH is used.
pub fn python(home: &Path) -> Process {
    let mut cmd = match std::env::var_os("CODIGEST_PY") {
        Some(py) => {
            let mut cmd = Process::new(py);
            cmd.args(["-m", "codigest"]);
            let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("src");
            cmd.env("PYTHONPATH", src);
            cmd
        }
        None => Process::new(codigest::runner::which("cdg").unwrap_or_else(|| "cdg".into())),
    };
    cmd.env("CODIGEST_HOME", home)
        .env("NO_COLOR", "1")
        .env("COLUMNS", "1000")
        .env("PYTHONIOENCODING", "utf-8")
        .env("PYTHONUTF8", "1")
        .env_remove("CODIGEST_HOOK")
        .env_remove("CODIGEST_SHELL");
    cmd
}

/// Parity tests are `#[ignore]`d; run them with `CODIGEST_PARITY=1 cargo test -- --ignored`.
/// Run without the variable they fail loudly instead of passing without comparing anything.
pub fn require_parity() {
    assert!(
        std::env::var("CODIGEST_PARITY").is_ok_and(|v| v == "1"),
        "parity tests compare against the Python build: set CODIGEST_PARITY=1"
    );
}

/// What a command printed, with the parts that legitimately differ between two runs
/// replaced: timestamps, durations, relative times.
pub fn normalise(text: &str) -> String {
    let patterns = [
        (r"\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d[+-]\d\d:\d\d", "<TIME>"),
        (r#"duration="[^"]*""#, r#"duration="<D>""#),
        (r"\(\d+ \w+ ago\)", "(<AGO>)"),
    ];
    let mut out = text.replace("\r\n", "\n");
    for (pattern, replacement) in patterns {
        out = Regex::new(pattern)
            .expect("valid pattern")
            .replace_all(&out, replacement)
            .into_owned();
    }
    out
}

/// Shebang interpreter for stand-in scripts. Termux has no `/bin/sh`; its `sh` is under `$PREFIX`.
pub fn posix_sh() -> PathBuf {
    codigest::runner::which("sh").unwrap_or_else(|| "/bin/sh".into())
}

pub fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// A command that prints to both streams and exits with `code`, for the host's shell.
pub fn script(code: i32) -> Vec<String> {
    if cfg!(windows) {
        vec![
            "cmd".into(),
            "/c".into(),
            format!("echo to-out& echo to-err 1>&2& exit {code}"),
        ]
    } else {
        vec![
            "sh".into(),
            "-c".into(),
            format!("echo to-out; echo to-err >&2; exit {code}"),
        ]
    }
}
