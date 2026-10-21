//! `term --format` and `--preset`, which only the Rust build has.

mod common;

use common::Fixture;

fn with_history() -> Fixture {
    let fx = Fixture::new();
    let lines = [
        r#"{"command": "pytest -q", "output": "1 failed\nuse ``` here", "exit_code": 1, "cwd": "/w", "started_at": "2026-01-01T10:00:00+09:00", "duration_ms": 1500}"#,
        r#"{"command": "echo hi", "output": "hi", "exit_code": 0, "cwd": "/w", "started_at": "2026-01-01T10:00:05+09:00"}"#,
    ];
    let dir = fx.home.join("term");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("history.jsonl"), lines.join("\n") + "\n").unwrap();
    fx
}

#[test]
fn markdown_puts_each_command_and_its_output_in_one_console_block() {
    let fx = with_history();
    let run = fx.rust(&["term", "-n", "2", "--format", "markdown", "-s"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(
        run.stdout.trim_end(),
        "## Command 2 (exit 1)\n\n- cwd: /w\n- duration: 1.5s\n- at: 2026-01-01T10:00:00+09:00\n\n\
         ````console\n$ pytest -q\n1 failed\nuse ``` here\n````\n\n\
         ## Command 1 (exit 0)\n\n- cwd: /w\n- at: 2026-01-01T10:00:05+09:00\n\n\
         ```console\n$ echo hi\nhi\n```"
    );
}

#[test]
fn json_is_an_array_indexed_from_the_latest() {
    let fx = with_history();
    let run = fx.rust(&["term", "-n", "2", "--format", "json", "-s"]);
    let items: serde_json::Value = serde_json::from_str(&run.stdout).unwrap();
    assert_eq!(items[0]["index"], 2);
    assert_eq!(items[0]["exit_code"], 1);
    assert_eq!(items[1]["command"], "echo hi");
    assert!(items[0].get("hostname").is_none());
}

#[test]
fn a_preset_fills_options_and_the_command_line_overrides_them() {
    let fx = with_history();
    fx.write(
        ".codigest/config.toml",
        "[presets.term.both]\nformat = \"json\"\ncount = 2\nstdout = true\n",
    );
    let run = fx.rust(&["term", "--preset", "both"]);
    let items: serde_json::Value = serde_json::from_str(&run.stdout).unwrap();
    assert_eq!(items.as_array().unwrap().len(), 2);

    let run = fx.rust(&["term", "--preset=both", "-n", "1"]);
    let items: serde_json::Value = serde_json::from_str(&run.stdout).unwrap();
    assert_eq!(items.as_array().unwrap().len(), 1);

    let run = fx.rust(&["term", "--preset", "missing"]);
    assert_eq!(run.code, 2);
    assert!(run.stderr.contains("no preset \"missing\" for term"));
}
