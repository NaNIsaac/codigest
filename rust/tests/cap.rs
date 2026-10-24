#[path = "capture_common/mod.rs"]
mod common;

use std::fs;
use std::path::Path;

use codigest::runner::{join_command, which};
use common::{normalise, python, require_parity, rust, rust_exe, script, stdout_of};
use serde_json::Value;

fn history(home: &Path) -> Vec<Value> {
    let text = fs::read_to_string(home.join("term").join("history.jsonl")).unwrap_or_default();
    text.lines()
        .map(|line| serde_json::from_str(line).expect("a JSON line"))
        .collect()
}

#[test]
fn a_command_that_took_under_a_millisecond_compares_equal_to_one_that_took_longer() {
    let quick = r#"<command index="1" exit_code="1" at="2026-10-17T01:58:14+09:00">"#;
    let slower =
        r#"<command index="1" exit_code="1" duration="1ms" at="2026-10-17T02:00:00+09:00">"#;
    let longer =
        r#"<command index="1" exit_code="1" duration="1.5s" at="2026-10-17T02:00:00+09:00">"#;
    assert_eq!(normalise(quick), normalise(slower));
    assert_eq!(normalise(quick), normalise(longer));
    assert_eq!(
        normalise(quick),
        r#"<command index="1" exit_code="1" at="<TIME>">"#
    );
}

#[test]
fn output_from_both_streams_is_shown_and_stored_and_the_exit_code_passes_through() {
    let home = tempfile::tempdir().unwrap();
    let program = script(3);

    let output = rust(home.path())
        .arg("cap")
        .arg("--")
        .args(&program)
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(3));
    let shown = stdout_of(&output);
    assert!(
        shown.contains("to-out") && shown.contains("to-err"),
        "{shown}"
    );
    assert!(output.stderr.is_empty(), "stderr is merged into stdout");

    let entries = history(home.path());
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["command"], join_command(&program));
    assert_eq!(entries[0]["exit_code"], 3);
    assert!(entries[0]["output"].as_str().unwrap().contains("to-err"));
    assert_eq!(
        entries[0]["cwd"],
        std::env::current_dir().unwrap().to_string_lossy().as_ref()
    );
    assert!(!entries[0]["shell"].as_str().unwrap().is_empty());
    assert!(
        entries[0]["hostname"]
            .as_str()
            .is_some_and(|h| !h.is_empty())
    );
}

#[test]
fn flags_after_cap_belong_to_the_wrapped_command() {
    let home = tempfile::tempdir().unwrap();
    let exe = rust_exe();

    let output = rust(home.path())
        .arg("cap")
        .arg(&exe)
        .arg("--version")
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(stdout_of(&output).contains(env!("CARGO_PKG_VERSION")));
    assert_eq!(history(home.path()).len(), 1);
}

#[test]
fn no_command_prints_usage_and_exits_2() {
    let home = tempfile::tempdir().unwrap();
    let output = rust(home.path()).arg("cap").output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("cdg cap -- <command>"));
    assert!(history(home.path()).is_empty());
}

#[test]
fn a_missing_program_exits_127_and_stores_nothing() {
    let home = tempfile::tempdir().unwrap();
    let output = rust(home.path())
        .args(["cap", "--", "codigest-no-such-program-xyz"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(127));
    assert!(String::from_utf8_lossy(&output.stderr).contains("Command not found"));
    assert!(history(home.path()).is_empty());
}

fn has_pipeline_shell() -> bool {
    !cfg!(windows) || which("pwsh").or_else(|| which("powershell")).is_some()
}

#[test]
fn one_argument_with_metacharacters_runs_through_a_shell() {
    if !has_pipeline_shell() {
        return;
    }
    let home = tempfile::tempdir().unwrap();
    let output = rust(home.path())
        .args(["cap", "echo one; echo two"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let lines: Vec<String> = stdout_of(&output)
        .lines()
        .map(|l| l.trim().to_string())
        .collect();
    assert_eq!(lines, ["one", "two"]);
    assert_eq!(history(home.path())[0]["command"], "echo one; echo two");
}

#[test]
fn powershell_cmdlets_run_on_windows() {
    if !cfg!(windows) || !has_pipeline_shell() {
        return;
    }
    let home = tempfile::tempdir().unwrap();
    let output = rust(home.path())
        .args(["cap", "Write-Output", "a b", "it's"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let lines: Vec<String> = stdout_of(&output)
        .lines()
        .map(|l| l.trim().to_string())
        .collect();
    assert_eq!(lines, ["a b", "it's"]);
    assert_eq!(history(home.path()).len(), 1);

    // A cmdlet, not `ls`: CI runners have Git's ls.exe on PATH, and an executable wins.
    let listing = rust(home.path())
        .args(["cap", "Get-ChildItem", "-Name"])
        .output()
        .unwrap();
    assert!(listing.status.success());
}

#[test]
fn secrets_are_masked_in_the_store_but_shown_live() {
    let home = tempfile::tempdir().unwrap();
    let echo = if cfg!(windows) {
        vec!["cmd", "/c", "echo", "password=hunter22xyz"]
    } else {
        vec!["echo", "password=hunter22xyz"]
    };
    let output = rust(home.path())
        .arg("cap")
        .arg("--")
        .args(&echo)
        .output()
        .unwrap();
    assert!(stdout_of(&output).contains("hunter22xyz"));
    let stored = history(home.path());
    assert_eq!(stored[0]["output"], "password=[REDACTED]");
}

#[cfg(unix)]
#[test]
fn a_signal_death_exits_like_the_python_build() {
    let home = tempfile::tempdir().unwrap();
    let output = rust(home.path())
        .args(["cap", "--", "sh", "-c", "kill -9 $$"])
        .output()
        .unwrap();
    // Python returns -9 and `sys.exit(-9)` becomes 247.
    assert_eq!(output.status.code(), Some(247));
    assert_eq!(history(home.path())[0]["exit_code"], -9);
}

/// The stored entry without the fields that differ from run to run.
fn stable(mut entry: Value) -> Value {
    let map = entry.as_object_mut().unwrap();
    for key in ["started_at", "duration_ms"] {
        map.remove(key);
    }
    entry
}

fn compare_cap(args: &[&str]) {
    let (rs_home, py_home) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let rs = rust(rs_home.path()).arg("cap").args(args).output().unwrap();
    let py = python(py_home.path())
        .arg("cap")
        .args(args)
        .output()
        .unwrap();

    assert_eq!(rs.status.code(), py.status.code(), "exit code for {args:?}");
    assert_eq!(
        normalise(&stdout_of(&rs)),
        normalise(&String::from_utf8_lossy(&py.stdout)),
        "stdout for {args:?}"
    );

    let (rs_entries, py_entries) = (history(rs_home.path()), history(py_home.path()));
    assert_eq!(
        rs_entries.len(),
        py_entries.len(),
        "entry count for {args:?}"
    );
    for (r, p) in rs_entries.into_iter().zip(py_entries) {
        assert_eq!(stable(r), stable(p), "stored entry for {args:?}");
    }
}

#[test]
#[ignore = "needs CODIGEST_PARITY=1 and the Python cdg"]
fn parity_cap_exit_code_streams_and_stored_entry() {
    require_parity();
    let program = script(3);
    let args: Vec<&str> = program.iter().map(String::as_str).collect();
    let mut with_dashes = vec!["--"];
    with_dashes.extend(args);
    compare_cap(&with_dashes);
}

#[test]
#[ignore = "needs CODIGEST_PARITY=1 and the Python cdg"]
fn parity_cap_secrets_and_missing_program() {
    require_parity();
    let echo: Vec<&str> = if cfg!(windows) {
        vec!["--", "cmd", "/c", "echo", "password=hunter22xyz"]
    } else {
        vec!["--", "echo", "password=hunter22xyz"]
    };
    compare_cap(&echo);
    compare_cap(&["--", "codigest-no-such-program-xyz"]);
    compare_cap(&[]);
}

#[test]
#[ignore = "needs CODIGEST_PARITY=1 and the Python cdg"]
fn parity_cap_pipeline_in_one_argument() {
    require_parity();
    if !has_pipeline_shell() {
        return;
    }
    compare_cap(&["echo one; echo two"]);
}

#[test]
#[ignore = "needs CODIGEST_PARITY=1 and the Python cdg"]
fn parity_cap_powershell_cmdlet() {
    require_parity();
    if !cfg!(windows) || !has_pipeline_shell() {
        return;
    }
    compare_cap(&["Write-Output", "a b", "it's"]);
}

#[test]
#[ignore = "needs CODIGEST_PARITY=1 and the Python cdg"]
fn parity_term_renders_what_either_build_stored() {
    require_parity();
    let program = script(1);
    let (rs_home, py_home) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    rust(rs_home.path())
        .arg("cap")
        .arg("--")
        .args(&program)
        .assert()
        .code(1);
    python(py_home.path())
        .arg("cap")
        .arg("--")
        .args(&program)
        .output()
        .unwrap();

    // Each build reads the other's store, which is the point of sharing the format.
    let from_rs = rust(rs_home.path())
        .args(["term", "--context", "-s"])
        .output()
        .unwrap();
    let from_py = python(py_home.path())
        .args(["term", "--context", "-s"])
        .output()
        .unwrap();
    let cross = rust(py_home.path())
        .args(["term", "--context", "-s"])
        .output()
        .unwrap();

    assert_eq!(
        normalise(&stdout_of(&from_rs)),
        normalise(&String::from_utf8_lossy(&from_py.stdout))
    );
    assert_eq!(
        normalise(&stdout_of(&cross)),
        normalise(&String::from_utf8_lossy(&from_py.stdout))
    );
}

#[test]
fn options_before_the_command_hand_over_the_stored_entry() {
    let home = tempfile::tempdir().unwrap();
    let program = script(0);

    let output = rust(home.path())
        .args(["cap", "--stdout", "--format", "json", "--"])
        .args(&program)
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(0));
    let shown = stdout_of(&output);
    // The live output first, then the entry as `cdg term --format json` would print it.
    assert!(
        shown.find("to-out").unwrap() < shown.find("\"exit_code\": 0").unwrap(),
        "{shown}"
    );
    assert_eq!(history(home.path()).len(), 1);
}

#[test]
#[ignore = "needs CODIGEST_PARITY=1 and the Python cdg"]
fn parity_cap_hands_over_the_entry() {
    require_parity();
    let program = script(3);
    let mut args = vec!["--stdout", "--"];
    args.extend(program.iter().map(String::as_str));
    compare_cap(&args);
}
