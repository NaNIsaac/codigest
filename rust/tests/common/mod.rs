//! Shared helpers of the integration tests: a project fixture and a way to run both builds.

#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use tempfile::TempDir;

pub const RUST: &str = env!("CARGO_BIN_EXE_cdg-rs");

/// A parent folder holding `proj`, so two fixtures can have different paths and the same
/// project name.
pub struct Fixture {
    _parent: TempDir,
    pub root: PathBuf,
    pub home: PathBuf,
}

impl Fixture {
    pub fn new() -> Self {
        let parent = tempfile::tempdir().unwrap();
        let root = parent.path().join("proj");
        let home = parent.path().join("home");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&home).unwrap();
        fs::create_dir(root.join(".git")).unwrap();
        Self {
            _parent: parent,
            root,
            home,
        }
    }

    /// The project most tests use: nesting, ignored files, a secret, odd encodings.
    pub fn sample() -> Self {
        let fx = Self::new();
        fx.write(
            ".gitignore",
            "# build output\n*.log\n!keep.log\nbuild/\n/todo.txt\n",
        );
        fx.write("README.md", "# demo\n\nsome text\n");
        fx.write("a.py", "import os\n\n\ndef main():\n    return 1\n");
        fx.write("B.py", "x = 1\n");
        fx.write("a-b/x.py", "y = 2\n");
        fx.write("a/x.py", "z = 3\n");
        fx.write("src/app.py", "class App:\n    pass\n</file>\n");
        fx.write("src/pkg/util.py", "def f(): ...\n");
        fx.write("src/pkg/data.json", "{\"k\": 1}\n");
        fx.write("src/notes.txt", "plain\n");
        fx.write("build/out.py", "ignored = True\n");
        fx.write("debug.log", "ignored\n");
        fx.write("keep.log", "kept\n");
        fx.write("todo.txt", "ignored at the root\n");
        fx.write("src/todo.txt", "kept below\n");
        fx.write(".env", "TOKEN=abc\n");
        fx.write("Dockerfile", "FROM scratch\n");
        fx.write("pyproject.toml", "[project]\nname = \"demo\"\n");
        fx.write("empty.py", "");
        fx.write("crlf.py", "a = 1\r\nb = 2\r\n");
        fx.write("uni\u{ed}.py", "s = \"h\u{e9}llo \u{c1}\"\n");
        fx.write("ff.py", "a\x0cb\nc\n");
        fx.write("__pycache__/m.pyc", "x");
        fx.write("node_modules/p/i.js", "x");
        fx.bytes("bin.py", &[0xff, 0xfe, 0x00, 0x41]);
        fx
    }

    pub fn write(&self, rel: &str, text: &str) {
        self.bytes(rel, text.as_bytes());
    }

    pub fn bytes(&self, rel: &str, data: &[u8]) {
        let path = self.root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, data).unwrap();
    }

    pub fn config(&self, text: &str) {
        self.write(".codigest/config.toml", text);
    }

    pub fn read(&self, rel: &str) -> String {
        fs::read_to_string(self.root.join(rel)).unwrap()
    }

    /// Runs a build in the project with a clean, wide, colourless environment.
    pub fn run(&self, program: &str, args: &[&str], stdin: Option<&str>) -> Run {
        run_in(&self.root, &self.home, program, args, stdin)
    }

    pub fn rust(&self, args: &[&str]) -> Run {
        self.run(RUST, args, None)
    }

    pub fn python(&self, args: &[&str]) -> Run {
        self.run("cdg", args, None)
    }
}

pub struct Run {
    /// Stdout with `\r\n` turned into `\n`, for assertions that are about content.
    pub stdout: String,
    /// Stdout exactly as written, for comparing the two builds byte for byte.
    pub raw_stdout: String,
    pub stderr: String,
    pub code: i32,
}

pub fn run_in(cwd: &Path, home: &Path, program: &str, args: &[&str], stdin: Option<&str>) -> Run {
    use std::io::Write;
    let mut child = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .env("CODIGEST_HOME", home)
        .env("NO_COLOR", "1")
        .env("COLUMNS", "500")
        .env("PYTHONIOENCODING", "utf-8")
        .env("PYTHONUTF8", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("cannot start {program}: {e}"));
    if let Some(text) = stdin {
        child
            .stdin
            .take()
            .unwrap()
            .write_all(text.as_bytes())
            .unwrap();
    }
    drop(child.stdin.take());
    let Output {
        status,
        stdout,
        stderr,
    } = child.wait_with_output().unwrap();
    let raw_stdout = String::from_utf8_lossy(&stdout).into_owned();
    Run {
        stdout: lf(&raw_stdout),
        raw_stdout,
        stderr: String::from_utf8_lossy(&stderr).into_owned(),
        code: status.code().unwrap_or(-1),
    }
}

/// True when the parity tests were asked for. They need the Python `cdg` on PATH.
pub fn parity_enabled() -> bool {
    let on = std::env::var("CODIGEST_PARITY").is_ok_and(|v| v == "1");
    if !on {
        eprintln!("skipped: set CODIGEST_PARITY=1 to compare with the Python build");
    }
    on
}

/// Both builds write the platform's line ending; content assertions compare with `\n`.
pub fn lf(text: &str) -> String {
    text.replace("\r\n", "\n")
}

/// Runs both builds with the same arguments in the same project and compares stdout
/// byte for byte and the exit code.
pub fn assert_parity(fx: &Fixture, args: &[&str]) -> (Run, Run) {
    let python = fx.python(args);
    let rust = fx.rust(args);
    assert_eq!(
        python.raw_stdout, rust.raw_stdout,
        "stdout differs for {args:?}\npython stderr: {}\nrust stderr: {}",
        python.stderr, rust.stderr
    );
    assert_eq!(python.code, rust.code, "exit code differs for {args:?}");
    (python, rust)
}

/// The on-disk spelling of a path, without the `\?\` prefix `canonicalize` adds.
pub fn real_path(path: &Path) -> String {
    let real = fs::canonicalize(path).unwrap();
    codigest::paths::strip_verbatim(real).display().to_string()
}
