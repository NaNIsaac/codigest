//! `--format` of `term` and `scan`, and `--preset`.

mod common;

use common::{Fixture, assert_parity, lf, parity_enabled};

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

fn rich_history() -> Fixture {
    let fx = Fixture::new();
    let lines = [
        r#"{"command": "pytest -q \"tests/\"", "output": "1 failed\nuse ``` here\n</output>\ttab \u0001 café ☃", "exit_code": 1, "cwd": "/w", "shell": "bash", "hostname": "PC <1>", "started_at": "2026-01-01T10:00:00+09:00", "duration_ms": 1500, "truncated": true}"#,
        r#"{"command": "echo hi", "output": "", "exit_code": 0, "cwd": "", "started_at": "2026-01-01T10:00:05+09:00", "duration_ms": 12}"#,
        r#"{"command": "ls", "output": "", "captured": false, "note": "from <history>", "shell": "zsh"}"#,
        r#"{"command": "make", "output": "a\n````\nb", "exit_code": 2, "cwd": "/w/x", "duration_ms": 61000}"#,
    ];
    let dir = fx.home.join("term");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("history.jsonl"), lines.join("\n") + "\n").unwrap();
    fx
}

#[test]
fn parity_term_formats() {
    if !parity_enabled() {
        return;
    }
    let fx = rich_history();
    for format in ["xml", "markdown", "json"] {
        for extra in [
            vec!["-n", "4"],
            vec!["-n", "4", "--context"],
            vec!["-i", "3"],
            vec!["-f", "-n", "2"],
            vec!["-n", "4", "--context", "-o", "out.txt"],
        ] {
            let mut args = vec!["term", "-s", "--format", format];
            args.extend(extra);
            assert_parity(&fx, &args);
        }
    }
    assert_parity(&fx, &["term", "-s", "--format", "json", "--here"]);
    assert_parity(
        &fx,
        &[
            "term", "-s", "-n", "4", "--format", "json", "--format", "markdown",
        ],
    );
    assert_parity(
        &fx,
        &["term", "-s", "-n", "3", "-n", "1", "--format", "json"],
    );
}

#[test]
fn parity_scan_formats() {
    if !parity_enabled() {
        return;
    }
    let fx = Fixture::sample();
    fx.write(
        "fence.md",
        "# t\n\n```python\nx = 1\n```\n\n````\nlong\n````\n",
    );
    fx.write("noext", "no extension\n");
    fx.write("dir.d/Up.PY", "A = 1\n");
    for format in ["markdown", "json"] {
        for extra in [
            vec!["-y"],
            vec!["-y", "-l"],
            vec!["-y", "-a"],
            vec!["-y", "-m", "fix </file> & \"it\" \u{e9}"],
            vec!["src"],
            vec!["a.py:1-2", "src/pkg", "fence.md", "noext"],
            vec!["a.py:2", "a.py:4-9", "B.py:7-9"],
            vec!["dir.d", "fence.md", "-l"],
            vec!["-y", "--resolve", "a.py"],
        ] {
            let mut args = vec!["scan", "-s", "--format", format];
            args.extend(extra);
            assert_parity(&fx, &args);
        }
    }
    assert_parity(
        &fx,
        &[
            "scan", "-s", "-y", "--format", "json", "--format", "markdown",
        ],
    );
    assert_parity(&fx, &["scan", "-s", "-y", "--format", "xml", "src/app.py"]);
    assert!(!fx.root.join(".codigest").join("cache").exists());
}

#[test]
fn parity_presets() {
    if !parity_enabled() {
        return;
    }
    let fx = rich_history();
    fx.write(
        ".codigest/config.toml",
        "[presets.term.debug]\nformat = \"markdown\"\ncount = 3\nfailed = true\n\
         [presets.term.both]\nformat = \"json\"\ncount = 4\nstdout = true\ncontext = true\n\
         [presets.scan.chat]\nformat = \"markdown\"\nmessage = \"look\"\nyes = true\nline_numbers = true\nstdout = true\n\
         [presets.scan.off]\nformat = \"json\"\nall = false\nyes = true\nstdout = true\n\
         [presets.term.bad]\nnested = { a = 1 }\n\
         [presets.term.odd]\ncount = 2\nhere = false\nformat = [\"json\"]\n",
    );
    fx.write("a.py", "x = 1\n");
    for args in [
        vec!["term", "--preset", "debug", "-s"],
        vec!["term", "--preset=debug", "-s", "-n", "1"],
        vec!["term", "-s", "--preset", "both", "--format", "markdown"],
        vec!["term", "--preset", "both", "-n", "1"],
        vec!["scan", "--preset", "chat", "a.py"],
        vec!["scan", "a.py", "--preset", "chat", "-m", "other"],
        vec!["scan", "--preset", "off"],
        vec!["--preset", "both", "term"],
    ] {
        assert_parity(&fx, &args);
    }
    for args in [
        vec!["term", "--preset", "missing"],
        vec!["scan", "--preset", "debug"],
        vec!["term", "--preset"],
        vec!["term", "--preset", "bad"],
    ] {
        let (python, rust) = assert_parity(&fx, &args);
        assert_eq!(python.code, 2, "{args:?}");
        assert_eq!(lf(&python.stderr), lf(&rust.stderr), "{args:?}");
    }
}
