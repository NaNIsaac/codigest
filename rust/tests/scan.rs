mod common;

use common::{Fixture, assert_parity, parity_enabled};

fn files_in(snapshot: &str) -> Vec<String> {
    snapshot
        .lines()
        .filter_map(|l| l.strip_prefix("<file path=\""))
        .map(|l| l.split('"').next().unwrap().to_owned())
        .collect()
}

#[test]
fn scan_shares_the_project_in_pathlib_order() {
    let fx = Fixture::sample();
    let run = fx.rust(&["scan"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert!(
        run.stdout
            .starts_with("<project_snapshot project=\"proj\">\n<project_structure>\n")
    );
    assert!(
        run.stdout
            .ends_with("</source_code>\n</project_snapshot>\n")
    );
    let files = files_in(&run.stdout);
    // Part by part and case-folded on Windows, whole-string elsewhere only by parts too.
    let position = |name: &str| files.iter().position(|f| f == name).unwrap();
    assert!(position("a/x.py") < position("a-b/x.py"));
    assert!(files.contains(&".gitignore".to_owned()));
    assert!(files.contains(&"Dockerfile".to_owned()));
    for gone in [
        ".env",
        "debug.log",
        "todo.txt",
        "build/out.py",
        "__pycache__/m.pyc",
    ] {
        assert!(!files.contains(&gone.to_owned()), "{gone} was shared");
    }
    assert!(files.contains(&"keep.log".to_owned()));
    assert!(files.contains(&"src/todo.txt".to_owned()));
    assert!(run.stderr.contains("Withheld (may contain secrets): .env"));
}

#[test]
fn scan_escapes_closing_tags_and_reads_text_mode() {
    let fx = Fixture::sample();
    let out = fx.rust(&["scan"]).stdout;
    assert!(out.contains("&lt;/file>\n</file>"));
    assert!(out.contains("<file path=\"crlf.py\">\na = 1\nb = 2\n</file>"));
    assert!(out.contains("<file path=\"bin.py\">\n<<Binary or Non-UTF8 Content>>\n</file>"));
    assert!(out.contains("<file path=\"empty.py\">\n\n</file>"));
}

#[test]
fn scan_numbers_lines_and_takes_ranges() {
    let fx = Fixture::sample();
    let out = fx.rust(&["scan", "-l", "a.py"]).stdout;
    assert!(out.contains(
        "<file path=\"a.py\">\n1: import os\n2: \n3: \n4: def main():\n5:     return 1\n</file>"
    ));
    let out = fx.rust(&["scan", "a.py:4-5", "src/pkg/util.py:1"]).stdout;
    assert!(
        out.contains(
            "<file path=\"a.py\" lines=\"4-5\">\n4: def main():\n5:     return 1\n</file>"
        )
    );
    assert!(out.contains("<file path=\"src/pkg/util.py\" lines=\"1\">\n1: def f(): ...\n</file>"));
}

#[test]
fn scan_message_goes_in_an_escaped_instruction_block() {
    let fx = Fixture::sample();
    let out = fx.rust(&["scan", "a.py", "-m", "why </file>?"]).stdout;
    assert!(
        out.contains(
            "<user_instruction>\nwhy &lt;/file>?\n</user_instruction>\n<project_structure>"
        )
    );
}

#[test]
fn scan_bad_target_is_a_usage_error() {
    let fx = Fixture::sample();
    let run = fx.rust(&["scan", "nope.py:3"]);
    assert_eq!(run.code, 2);
    assert!(
        run.stderr
            .contains("nope.py:3 does not exist (a line range is FILE:START-END or FILE:LINE)")
    );
}

#[test]
fn a_large_project_stops_when_nobody_can_answer() {
    let fx = Fixture::new();
    fx.write("big.py", &"x = 1\n".repeat(25_000));
    let run = fx.rust(&["scan"]);
    assert_eq!(run.code, 1);
    assert!(
        run.stderr
            .contains("Large: 1 files, 150.0 kB, about 37,500 tokens")
    );
    assert!(run.stderr.contains("Stopped. Share less by naming paths"));
    assert!(run.stdout.is_empty());
    let run = fx.rust(&["scan", "-y"]);
    assert_eq!(run.code, 0);
    assert!(run.stdout.starts_with("<project_snapshot"));
}

#[test]
fn config_filters_apply_and_all_ignores_them() {
    let fx = Fixture::sample();
    fx.config("[filter]\nextensions = [\".py\"]\nexclude_patterns = [\"src/\"]\n");
    let files = files_in(&fx.rust(&["scan"]).stdout);
    assert!(files.iter().all(|f| f.ends_with(".py")
        || f == ".gitignore"
        || f == "Dockerfile"
        || f == "pyproject.toml"));
    assert!(!files.iter().any(|f| f.starts_with("src/")));
    let files = files_in(&fx.rust(&["scan", "-a"]).stdout);
    assert!(files.contains(&"src/app.py".to_owned()));
}

#[test]
fn the_cache_is_written_only_where_codigest_exists_and_reused() {
    let fx = Fixture::sample();
    fx.rust(&["scan", "a.py"]);
    assert!(!fx.root.join(".codigest").exists());

    fx.config("");
    let first = fx.rust(&["scan", "a.py"]).stdout;
    let cache = fx.read(".codigest/cache.json");
    assert!(cache.contains("\"format\": 2"));
    assert!(cache.contains("\"False\""));
    let second = fx.rust(&["scan", "a.py"]).stdout;
    assert_eq!(first, second);
    fx.rust(&["scan", "-l", "a.py"]);
    assert!(fx.read(".codigest/cache.json").contains("\"True\""));
}

#[test]
fn prompts_toml_overrides_the_layout() {
    let fx = Fixture::new();
    fx.write("a.py", "x\n");
    fx.write(
        ".codigest/prompts.toml",
        "[prompts]\nsnapshot = \"P:{project_name}\\n{source_code}\\n{{ok}}\"\n",
    );
    assert_eq!(
        fx.rust(&["scan"]).stdout,
        "P:proj\n<file path=\"a.py\">\nx\n</file>\n{ok}\n"
    );
    fx.write(
        ".codigest/prompts.toml",
        "[prompts]\nsnapshot = \"{nope}\"\n",
    );
    assert_eq!(
        fx.rust(&["scan"]).stdout,
        "Error rendering template 'snapshot': Missing argument 'nope'\n"
    );
    fx.write(".codigest/prompts.toml", "[prompts]\nsnapshot = \"}\"\n");
    assert_eq!(fx.rust(&["scan"]).code, 1);
}

#[test]
fn output_file_gets_the_snapshot() {
    let fx = Fixture::new();
    fx.write("a.py", "x\n");
    let run = fx.rust(&["scan", "-o", "out.txt"]);
    assert_eq!(run.code, 0);
    let text = fx.read("out.txt");
    assert!(text.starts_with("<project_snapshot"));
    assert_eq!(text, run.raw_stdout);
}

#[test]
fn parity_scan() {
    if !parity_enabled() {
        return;
    }
    let fx = Fixture::sample();
    for args in [
        vec!["scan"],
        vec!["scan", "-l"],
        vec!["scan", "-a"],
        vec!["scan", "-m", "fix </file> & <b>"],
        vec!["scan", "src"],
        vec!["scan", "src/pkg", "a.py", "B.py"],
        vec![
            "scan",
            "a.py:2-4",
            "a.py:5",
            "src/app.py:2-99",
            "src/app.py:50",
        ],
        vec!["scan", "ff.py", "-l"],
        vec!["scan", "empty.py", "-l"],
        vec!["scan", "nope.py:3"],
        vec!["scan", "-y"],
    ] {
        assert_parity(&fx, &args);
    }
    fx.config(
        "[filter]\nextensions = [\".py\", \".md\"]\nexclude_patterns = [\"a-b/\", \"*.json\"]\n",
    );
    assert_parity(&fx, &["scan"]);
    assert_parity(&fx, &["scan", "-a"]);
    fx.write(
        ".codigest/prompts.toml",
        "[prompts]\nsnapshot = \"<<{project_name}|{instruction:>6}|{tree_structure!r}>>{{}}\"\n",
    );
    assert_parity(&fx, &["scan", "src/pkg", "-m", "hi"]);
    fx.write(
        ".codigest/prompts.toml",
        "[prompts]\nsnapshot = \"{missing}\"\n",
    );
    assert_parity(&fx, &["scan", "a.py"]);
    fx.write(".codigest/prompts.toml", "[prompts]\nsnapshot = \"{}\"\n");
    assert_eq!(fx.python(&["scan", "a.py"]).code, 1);
    assert_eq!(fx.rust(&["scan", "a.py"]).code, 1);
}

#[test]
fn parity_scan_large_and_cache() {
    if !parity_enabled() {
        return;
    }
    let fx = Fixture::new();
    fx.write("big.py", &"x = 1\n".repeat(25_000));
    let (python, rust) = assert_parity(&fx, &["scan"]);
    assert_eq!(python.code, 1);
    assert_eq!(rust.code, 1);
    assert_parity(&fx, &["scan", "-y"]);

    // Each build reads what the other stored.
    let fx = Fixture::sample();
    fx.config("");
    let python = fx.python(&["scan", "-l", "src"]);
    let cached = fx.read(".codigest/cache.json");
    let rust = fx.rust(&["scan", "-l", "src"]);
    assert_eq!(common::lf(&python.stdout), rust.stdout);
    let after: serde_json::Value = serde_json::from_str(&fx.read(".codigest/cache.json")).unwrap();
    let before: serde_json::Value = serde_json::from_str(&cached).unwrap();
    assert_eq!(before, after, "a warm cache is left as it was");

    let fx = Fixture::sample();
    fx.config("");
    let rust = fx.rust(&["scan"]);
    let written: serde_json::Value =
        serde_json::from_str(&fx.read(".codigest/cache.json")).unwrap();
    let python = fx.python(&["scan"]);
    assert_eq!(common::lf(&python.stdout), rust.stdout);
    let after: serde_json::Value = serde_json::from_str(&fx.read(".codigest/cache.json")).unwrap();
    assert_eq!(
        written, after,
        "python rewrites the cache rust wrote to the same content"
    );
}
