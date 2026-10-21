//! The context anchor, against real Git. The parity tests make an anchor with one build and
//! read it with the other.

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use codigest::shadow::ContextAnchor;
use common::require_parity;
use serde_json::{Value, json};

fn write(root: &Path, rel: &str, content: &[u8]) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

/// A project directory with the files an anchor has to cope with.
fn project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(root, ".git/config", b"[core]\n");
    write(root, "a.py", b"x = 1\n");
    write(root, "sub/b.txt", b"hello\r\nworld\r\n");
    write(root, "my dir/c.md", b"# title\n");
    write(root, "한글.txt", b"text\n");
    write(root, ".gitignore", b"*.log\n");
    write(root, ".github/workflows/ci.yml", b"on: push\n");
    write(root, "bin.dat", &[0, 1, 2, 3, 255]);
    dir
}

fn files(root: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            let name = entry.file_name();
            if name == ".git" || name == ".codigest" {
                continue;
            }
            if path.is_dir() {
                walk(&path, out);
            } else {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    walk(root, &mut out);
    out.sort_by_key(|p| {
        p.components()
            .map(|c| c.as_os_str().to_string_lossy().to_lowercase())
            .collect::<Vec<_>>()
    });
    out
}

fn anchor(dir: &tempfile::TempDir) -> ContextAnchor {
    ContextAnchor::new(dir.path())
}

#[test]
fn an_unchanged_project_has_no_changes() {
    let dir = project();
    let a = anchor(&dir);
    assert_eq!(a.last_update_time(), "Never");
    assert!(!a.has_history());

    a.update(&files(&a.root)).unwrap();

    assert!(a.has_history());
    assert!(!a.has_changes(&files(&a.root)));
    assert_eq!(a.get_changes(&files(&a.root)).unwrap(), "");
    assert!(
        a.last_update_time().ends_with("ago"),
        "{}",
        a.last_update_time()
    );
    assert_eq!(a.read_anchor_file(Path::new("a.py")), "x = 1\n");
}

#[test]
fn edits_additions_and_deletions_show_as_a_diff() {
    let dir = project();
    let a = anchor(&dir);
    a.update(&files(&a.root)).unwrap();

    write(&a.root, "a.py", b"x = 2\n");
    write(&a.root, "new.py", b"y = 1\n");
    fs::remove_file(a.root.join("my dir").join("c.md")).unwrap();

    let current = files(&a.root);
    assert!(a.has_changes(&current));
    let diff = a.get_changes(&current).unwrap();
    assert!(diff.contains("diff --git a/a.py b/a.py"), "{diff}");
    assert!(diff.contains("-x = 1") && diff.contains("+x = 2"));
    assert!(diff.contains("diff --git a/new.py b/new.py") && diff.contains("new file mode"));
    assert!(diff.contains("my dir/c.md") && diff.contains("deleted file mode"));
    assert!(!diff.contains("temp_diff"));
    assert!(!diff.contains('\r'), "line endings are read as text");

    let changed: Vec<String> = a
        .get_changed_files(&current)
        .unwrap()
        .iter()
        .map(|p| {
            p.strip_prefix(&a.root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect();
    // "my dir/c.md" comes back as "dir/c.md": the last word of the `diff --git` line, as in Python.
    assert_eq!(changed, ["a.py", "dir/c.md", "new.py"]);
}

#[test]
fn hunks_for_git_named_paths_are_left_out() {
    let dir = project();
    let a = anchor(&dir);
    a.update(&files(&a.root)).unwrap();

    write(&a.root, ".gitignore", b"*.log\n*.tmp\n");
    write(
        &a.root,
        ".github/workflows/ci.yml",
        b"on: [push, pull_request]\n",
    );
    write(&a.root, ".git/config", b"[core]\nbare = false\n");

    let diff = a.get_changes(&files(&a.root)).unwrap();
    assert_eq!(diff, "", "only .git-named paths changed");
}

#[test]
fn the_next_update_moves_the_baseline() {
    let dir = project();
    let a = anchor(&dir);
    a.update(&files(&a.root)).unwrap();
    write(&a.root, "a.py", b"x = 2\n");
    assert!(a.has_changes(&files(&a.root)));

    a.update(&files(&a.root)).unwrap();

    assert_eq!(a.read_anchor_file(Path::new("a.py")), "x = 2\n");
    assert_eq!(a.get_changes(&files(&a.root)).unwrap(), "");
    assert_eq!(a.read_anchor_file(Path::new("missing.py")), "");
}

#[test]
fn the_project_is_not_touched_and_git_status_ignores_the_anchor() {
    let dir = project();
    let a = anchor(&dir);
    a.update(&files(&a.root)).unwrap();
    let marker = fs::read_to_string(a.root.join(".codigest").join(".gitignore")).unwrap();
    assert_eq!(marker.replace("\r\n", "\n"), "*\n");
    assert!(a.root.join(".codigest/anchor/.shadow_git/HEAD").exists());
    assert!(!a.root.join(".codigest/temp_diff_current").exists());
    assert_eq!(fs::read(a.root.join("bin.dat")).unwrap(), [0, 1, 2, 3, 255]);
}

// --------------------------------------------------------------------------
// Parity with the Python build
// --------------------------------------------------------------------------

const SCRIPT: &str = r#"
import json, sys
from pathlib import Path
from codigest.core.shadow import ContextAnchor

root = Path(sys.argv[1])
files = sorted(
    p for p in root.rglob("*")
    if p.is_file() and not {".git", ".codigest"} & set(p.relative_to(root).parts)
)
anchor = ContextAnchor(root)
if sys.argv[2] == "update":
    anchor.update(files)
print(json.dumps({
    "history": anchor.has_history(),
    "has_changes": anchor.has_changes(files),
    "last": anchor.get_last_update_time(),
    "changes": anchor.get_changes(files),
    "changed": [str(p.relative_to(anchor.root)) for p in anchor.get_changed_files(files)],
    "a_py": anchor.read_anchor_file(Path("a.py")),
}))
"#;

fn python_state(root: &Path, mode: &str) -> Value {
    let py = std::env::var_os("CODIGEST_PY")
        .expect("shadow parity needs CODIGEST_PY: a Python that can import codigest");
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("src");
    let output = Command::new(py)
        .args(["-c", SCRIPT])
        .arg(root)
        .arg(mode)
        .env("PYTHONPATH", src)
        .env("PYTHONIOENCODING", "utf-8")
        .env("PYTHONUTF8", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn rust_state(root: &Path, update: bool) -> Value {
    let anchor = ContextAnchor::new(root);
    let files = files(&anchor.root);
    if update {
        anchor.update(&files).unwrap();
    }
    json!({
        "history": anchor.has_history(),
        "has_changes": anchor.has_changes(&files),
        "last": anchor.last_update_time(),
        "changes": anchor.get_changes(&files).unwrap(),
        "changed": anchor
            .get_changed_files(&files)
            .unwrap()
            .iter()
            .map(|p| p.strip_prefix(&anchor.root).unwrap().to_string_lossy().into_owned())
            .collect::<Vec<_>>(),
        "a_py": anchor.read_anchor_file(Path::new("a.py")),
    })
}

fn same(left: Value, right: Value, what: &str) {
    let scrub = |mut v: Value| {
        // "1 second ago" and "2 seconds ago" are the same moment to this test.
        let last = regex::Regex::new(r"^\d+ \w+ ago$")
            .unwrap()
            .replace(v["last"].as_str().unwrap(), "<AGO>")
            .into_owned();
        v["last"] = Value::String(last);
        let changed: Vec<Value> = v["changed"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| Value::String(p.as_str().unwrap().replace('\\', "/")))
            .collect();
        v["changed"] = Value::Array(changed);
        v
    };
    assert_eq!(scrub(left), scrub(right), "{what}");
}

/// Edits that exercise modified, added, removed, CRLF, binary and git-named files.
fn edit(root: &Path) {
    write(root, "a.py", b"x = 2\nprint(x)\n");
    write(root, "sub/b.txt", b"hello\r\nthere\r\nworld\r\n");
    write(root, "new.py", b"y = 1\n");
    write(root, "bin.dat", &[0, 1, 2, 9, 255]);
    write(root, ".gitignore", b"*.log\n*.tmp\n");
    write(root, ".github/workflows/ci.yml", b"on: [push]\n");
    fs::remove_file(root.join("my dir").join("c.md")).unwrap();
}

#[test]
#[ignore = "needs CODIGEST_PARITY=1, CODIGEST_PY and git"]
fn parity_an_anchor_made_here_is_read_by_python() {
    require_parity();
    let dir = project();
    let root = dir.path();
    rust_state(root, true);
    same(
        rust_state(root, false),
        python_state(root, "read"),
        "unchanged",
    );

    edit(root);
    let (here, there) = (rust_state(root, false), python_state(root, "read"));
    assert!(
        here["changes"]
            .as_str()
            .unwrap()
            .contains("diff --git a/a.py b/a.py")
    );
    same(here, there, "after edits");

    // Python moves the baseline; this build must agree about the result.
    let moved = python_state(root, "update");
    same(
        rust_state(root, false),
        moved,
        "after Python moved the baseline",
    );
}

#[test]
#[ignore = "needs CODIGEST_PARITY=1, CODIGEST_PY and git"]
fn parity_an_anchor_made_by_python_is_read_here() {
    require_parity();
    let dir = project();
    let root = dir.path();
    python_state(root, "update");
    same(
        rust_state(root, false),
        python_state(root, "read"),
        "unchanged",
    );

    edit(root);
    same(
        rust_state(root, false),
        python_state(root, "read"),
        "after edits",
    );

    let moved = rust_state(root, true);
    same(
        moved,
        python_state(root, "read"),
        "after this build moved the baseline",
    );
}
