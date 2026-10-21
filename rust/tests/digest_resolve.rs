mod common;

use common::{Fixture, assert_parity, parity_enabled};

fn files_in(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|l| l.strip_prefix("<file path=\""))
        .map(|l| l.split('"').next().unwrap().to_owned())
        .collect()
}

#[test]
fn digest_outlines_python_and_matlab_without_bodies() {
    let fx = Fixture::code();
    let run = fx.rust(&["digest"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert!(run.stdout.starts_with(
        "<code_digest project=\"proj\" contents=\"signatures and docstrings, no bodies\">\n"
    ));
    assert!(run.stdout.contains(
        "class App(Base)
"
    ));
    assert!(!run.stdout.contains("return helper.go"));
    let files = files_in(&run.stdout);
    for expected in ["main.py", "helper.py", "stubs.pyi", "calc.m"] {
        assert!(files.contains(&expected.to_owned()), "{expected} missing");
    }
    assert!(!files.contains(&"app.js".to_owned()));
    assert!(run.stderr.contains("Digest "), "{}", run.stderr);
}

#[test]
fn digest_limits_to_targets_and_takes_a_message() {
    let fx = Fixture::code();
    let run = fx.rust(&["digest", "pkg", "-m", "look </file>"]);
    let files = files_in(&run.stdout);
    assert_eq!(files, ["pkg/deep.py", "pkg/util.py"]);
    assert!(
        run.stdout
            .contains("<user_instruction>\nlook &lt;/file>\n</user_instruction>")
    );
    assert_eq!(fx.rust(&["digest", "missing"]).code, 2);
}

#[test]
fn digest_caches_summaries_where_codigest_exists() {
    let fx = Fixture::code();
    fx.config("");
    let first = fx.rust(&["digest", "helper.py"]).stdout;
    let cache = fx.read(".codigest/cache.json");
    assert!(cache.contains("\"summaries\""));
    assert!(cache.contains("\"summaries_meta\""));
    assert_eq!(fx.rust(&["digest", "helper.py"]).stdout, first);
}

#[test]
fn resolve_follows_imports_for_scan_tree_and_digest() {
    let fx = Fixture::code();
    let scan = fx.rust(&["scan", "main.py", "-r"]);
    assert_eq!(scan.code, 0, "{}", scan.stderr);
    assert_eq!(
        files_in(&scan.stdout),
        [
            "helper.py",
            "main.py",
            "pkg/__init__.py",
            "pkg/deep.py",
            "pkg/util.py"
        ]
    );
    let tree = fx.rust(&["tree", "main.py", "-r"]);
    assert!(tree.stdout.contains("helper.py"));
    assert!(tree.stdout.contains("deep.py"));
    assert!(!tree.stdout.contains("unused.py"));
    let digest = fx.rust(&["digest", "main.py", "-r"]);
    assert!(files_in(&digest.stdout).contains(&"pkg/deep.py".to_owned()));
}

#[test]
fn parity_digest() {
    if !parity_enabled() {
        return;
    }
    let fx = Fixture::code();
    for args in [
        vec!["digest"],
        vec!["digest", "main.py"],
        vec!["digest", "pkg", "calc.m", "stubs.pyi"],
        vec!["digest", "crlf.py", "broken.py", "empty.py", "latin.py"],
        vec!["digest", "-m", "why </file> & <b>"],
        vec!["digest", "app.js"],
        vec!["digest", "missing.py"],
    ] {
        assert_parity(&fx, &args);
    }
    fx.config("[filter]\nextensions = [\".py\"]\nexclude_patterns = [\"pkg/\"]\n");
    assert_parity(&fx, &["digest"]);
    fx.write(
        ".codigest/prompts.toml",
        "[prompts]\ndigest = \"{project_name}:{instruction}\\n{digest_content}\"\n",
    );
    assert_parity(&fx, &["digest", "helper.py", "-m", "x"]);

    // Summaries stored by one build are read by the other.
    let fx = Fixture::code();
    fx.config("");
    let python = fx.python(&["digest"]);
    let rust = fx.rust(&["digest"]);
    assert_eq!(python.stdout, rust.stdout);
    let fx = Fixture::code();
    fx.config("");
    let rust = fx.rust(&["digest"]);
    let python = fx.python(&["digest"]);
    assert_eq!(python.stdout, rust.stdout);
}

#[test]
fn parity_resolve() {
    if !parity_enabled() {
        return;
    }
    let fx = Fixture::code();
    for args in [
        vec!["scan", "main.py", "-r"],
        vec!["scan", "-r", "-l", "pkg/util.py"],
        vec!["scan", "app.js", "-r"],
        vec!["scan", "-r", "main.py:1-4", "app.js"],
        vec!["scan", "-r"],
        vec!["tree", "main.py", "-r"],
        vec!["tree", "app.js", "-r", "-L", "1"],
        vec!["tree", "-r"],
        vec!["digest", "main.py", "-r"],
        vec!["digest", "pkg/util.py", "-r"],
        vec!["digest", "-r"],
    ] {
        assert_parity(&fx, &args);
    }
    fx.config("[filter]\nextensions = [\".md\"]\n");
    assert_parity(&fx, &["scan", "main.py", "-r"]);
    assert_parity(&fx, &["tree", "main.py", "-r"]);
    assert_parity(&fx, &["digest", "main.py", "-r"]);
}
