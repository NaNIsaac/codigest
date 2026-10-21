//! Parity for the commands the other test files leave out: `pack`, `diff`, `semdiff` and
//! `status`, on one project with a baseline, edits and a stored failure.

mod common;

use common::{Fixture, parity_enabled};
use regex::Regex;

/// `assert_parity` with the age of the baseline ("3 seconds ago") masked: the two builds
/// run a moment apart, so that is the one part of the output that may differ.
fn assert_parity(fx: &Fixture, args: &[&str]) {
    let ago = Regex::new(r"\d+ \w+ ago").unwrap();
    let (python, rust) = (fx.python(args), fx.rust(args));
    assert_eq!(
        ago.replace_all(&python.raw_stdout, "<AGO>"),
        ago.replace_all(&rust.raw_stdout, "<AGO>"),
        "stdout differs for {args:?}\npython stderr: {}\nrust stderr: {}",
        python.stderr,
        rust.stderr
    );
    assert_eq!(python.code, rust.code, "exit code differs for {args:?}");
}

/// A failing command whose output points at a file of the project, as pytest's would.
const FAILURE: &str = "import sys\n\
print('Traceback (most recent call last):')\n\
print('  File \"src/app.py\", line 2, in run')\n\
print('    return helper(1)')\n\
print('  File \"src/pkg/util.py\", line 1, in f')\n\
print('AssertionError: boom')\n\
sys.exit(1)\n";

/// The sample project with a baseline, then edits of every kind `diff` and `semdiff` know.
fn edited() -> Fixture {
    let fx = Fixture::sample();
    fx.write(
        "src/app.py",
        "class App:\n    def run(self):\n        return 1\n",
    );
    fx.write(
        "src/pkg/util.py",
        "def f(x):\n    return x\n\n\ndef g():\n    return 2\n",
    );
    let mark = fx.rust(&["mark"]);
    assert_eq!(mark.code, 0, "mark failed: {}", mark.stderr);
    fx.write(
        "src/app.py",
        "class App:\n    def run(self):\n        return 2\n",
    );
    fx.write("src/pkg/util.py", "def f(x, y=0):\n    return x\n");
    fx.write("src/new.py", "def added():\n    pass\n");
    std::fs::remove_file(fx.root.join("B.py")).unwrap();
    fx
}

#[test]
fn parity_diff_semdiff_and_status_after_edits() {
    if !parity_enabled() {
        return;
    }
    let fx = edited();
    for args in [
        vec!["diff", "-s"],
        vec!["diff", "-s", "src"],
        vec!["semdiff", "-s"],
        vec!["semdiff", "-s", "src/pkg"],
        vec!["status"],
    ] {
        assert_parity(&fx, &args);
    }
}

#[test]
fn parity_pack_bundles_the_same_failure_files_and_changes() {
    if !parity_enabled() {
        return;
    }
    let fx = edited();
    let cap = fx.rust(&["cap", "--", "python", "-c", FAILURE]);
    assert_eq!(
        cap.code, 1,
        "the stored command should fail: {}",
        cap.stderr
    );
    for args in [
        vec!["pack", "-s"],
        vec!["pack", "-s", "-m", "why does this fail"],
        vec!["pack", "-s", "--with", "changes"],
        vec!["pack", "-s", "src/app.py", "--with", "failure,imports"],
    ] {
        assert_parity(&fx, &args);
    }
}
