//! The context anchor (`core/shadow.py`): a private Git repository under
//! `<root>/.codigest/anchor/.shadow_git` that holds a copy of the project as of the last
//! `cdg mark`, so `diff`, `semdiff` and `pack` can compare against it. Git itself does the
//! storing and the diffing, through the `git` CLI, so an anchor made by the Python build
//! is read by this one and the other way round.
//!
//! The behaviour follows the Python build, which decides what a diff contains.

use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fs::{self, File, FileTimes};
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::term::is_space;

const SHADOW_DIR: &str = ".shadow_git";
// Directories that belong to git, the anchor or Codigest and never to the project.
const INTERNAL_DIRS: [&str; 3] = [".git", ".codigest", SHADOW_DIR];
// 1 ms: file systems disagree about timestamp precision.
const MTIME_TOLERANCE_NS: i128 = 1_000_000;

pub struct ContextAnchor {
    pub root: PathBuf,
    pub anchor_dir: PathBuf,
    pub git_dir: PathBuf,
}

impl ContextAnchor {
    pub fn new(root_path: &Path) -> Self {
        let root = resolve(root_path);
        let anchor_dir = resolve(&root.join(".codigest").join("anchor"));
        let git_dir = resolve(&anchor_dir.join(SHADOW_DIR));
        Self {
            root,
            anchor_dir,
            git_dir,
        }
    }

    /// A valid anchor (a Git repository) exists.
    pub fn has_history(&self) -> bool {
        self.git_dir.exists() && self.git_dir.join("HEAD").exists()
    }

    fn git(&self, cwd: Option<&Path>) -> Command {
        let mut cmd = Command::new("git");
        cmd.arg("--git-dir")
            .arg(&self.git_dir)
            .arg("--work-tree")
            .arg(&self.anchor_dir)
            .current_dir(cwd.unwrap_or(&self.anchor_dir));
        cmd
    }

    /// Runs git against the anchor and returns its trimmed stdout; failures give "".
    fn run_git(&self, args: &[&str]) -> String {
        let _ = fs::create_dir_all(&self.anchor_dir);
        let mut cmd = self.git(None);
        cmd.args(args);
        text_output(&mut cmd).map_or_else(String::new, |(out, _)| trim(&out))
    }

    /// Whether any file was added, modified or deleted since the last anchor.
    pub fn has_changes(&self, current_files: &[PathBuf]) -> bool {
        if !self.has_history() || !self.anchor_dir.exists() {
            return true;
        }

        let mut current_rel: BTreeSet<PathBuf> = BTreeSet::new();
        for src in current_files {
            if skipped(src) {
                continue;
            }
            let Ok(rel) = src.strip_prefix(&self.root) else {
                return true;
            };
            current_rel.insert(rel.to_path_buf());
            let dest = self.anchor_dir.join(rel);
            if !dest.exists() {
                return true;
            }
            let (Ok(a), Ok(b)) = (fs::metadata(src), fs::metadata(&dest)) else {
                return true;
            };
            if a.len() != b.len() || mtime_ns(&a) - mtime_ns(&b) > MTIME_TOLERANCE_NS {
                return true;
            }
        }

        for file in walk(&self.anchor_dir) {
            if file.is_file()
                && !has_component(&file, SHADOW_DIR)
                && file.file_name() != Some(OsStr::new(".gitignore"))
                && let Ok(rel) = file.strip_prefix(&self.anchor_dir)
                && !current_rel.contains(rel)
            {
                return true;
            }
        }
        false
    }

    /// Copies the project into the anchor and commits it when anything changed.
    pub fn update(&self, source_files: &[PathBuf]) -> io::Result<()> {
        fs::create_dir_all(&self.anchor_dir)?;
        hide_from_git(&self.root.join(".codigest"))?;
        write_text(&self.anchor_dir.join(".gitignore"), ".shadow_git\n")?;

        if !self.has_history() {
            if self.git_dir.exists() {
                remove_tree(&self.git_dir)?;
            }
            let _ = Command::new("git")
                .args(["init", "--bare"])
                .arg(&self.git_dir)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            for (key, value) in [
                ("core.bare", "false"),
                ("user.email", "codigest@ai"),
                ("user.name", "Context Manager"),
                ("core.autocrlf", "false"),
                ("gc.auto", "0"),
            ] {
                self.run_git(&["config", key, value]);
            }
        }

        // Nothing changed since the last anchor: skip the work.
        if self.has_history() && !self.has_changes(source_files) {
            return Ok(());
        }

        let mut source_rel: BTreeSet<PathBuf> = BTreeSet::new();
        for src in source_files {
            if skipped(src) {
                continue;
            }
            let Ok(rel) = src.strip_prefix(&self.root) else {
                continue;
            };
            let dest = self.anchor_dir.join(rel);
            let Some(parent) = dest.parent() else {
                continue;
            };
            if fs::create_dir_all(parent).is_err() {
                continue;
            }
            let Ok(src_meta) = fs::metadata(src) else {
                continue;
            };
            let mut should_copy = true;
            if dest.exists() {
                let Ok(dest_meta) = fs::metadata(&dest) else {
                    continue;
                };
                if src_meta.len() == dest_meta.len() && mtime_ns(&dest_meta) >= mtime_ns(&src_meta)
                {
                    should_copy = false;
                }
            }
            source_rel.insert(rel.to_path_buf());
            if should_copy {
                let _ = copy2(src, &dest);
            }
        }

        // Files that left the project leave the anchor (its own .gitignore included).
        for file in walk(&self.anchor_dir) {
            if file.is_file()
                && !has_component(&file, SHADOW_DIR)
                && let Ok(rel) = file.strip_prefix(&self.anchor_dir)
                && !source_rel.contains(rel)
            {
                let _ = fs::remove_file(&file);
            }
        }

        self.run_git(&["add", "."]);
        let snapshot = format!("Snapshot: {}", unix_seconds());
        if self.run_git(&["rev-parse", "--verify", "HEAD"]).is_empty() {
            self.run_git(&["commit", "-m", &snapshot]);
        } else {
            // --quiet reports changes only through the exit code, never on stdout.
            let clean = self
                .git(None)
                .args(["diff-index", "--quiet", "--cached", "HEAD", "--"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .is_ok_and(|status| status.success());
            if !clean {
                self.run_git(&["commit", "-m", &snapshot]);
            }
        }
        Ok(())
    }

    /// The unified diff from the anchor to the working tree, or "" when nothing changed.
    /// Sections about git's or the anchor's own directories are left out, judged by whole
    /// path components, so `.gitignore` and `.github/` are ordinary project files.
    pub fn get_changes(&self, current_files: &[PathBuf]) -> anyhow::Result<String> {
        if !self.git_dir.exists() || !self.has_changes(current_files) {
            return Ok(String::new());
        }

        let codigest = self.root.join(".codigest");
        let temp_current = codigest.join("temp_diff_current");
        let temp_baseline = codigest.join("temp_diff_baseline");
        for dir in [&temp_current, &temp_baseline] {
            if dir.exists() {
                let _ = remove_tree(dir);
            }
            fs::create_dir_all(dir)?;
        }

        let diff = self.diff_against(current_files, &codigest, &temp_current, &temp_baseline);

        for dir in [&temp_current, &temp_baseline] {
            if dir.exists() {
                let _ = remove_tree(dir);
            }
        }
        Ok(diff)
    }

    fn diff_against(
        &self,
        current_files: &[PathBuf],
        codigest: &Path,
        temp_current: &Path,
        temp_baseline: &Path,
    ) -> String {
        let mut current_rel: BTreeSet<PathBuf> = BTreeSet::new();
        for src in current_files {
            if skipped(src) {
                continue;
            }
            let Ok(rel) = src.strip_prefix(&self.root) else {
                continue;
            };
            current_rel.insert(rel.to_path_buf());
            let dest = temp_current.join(rel);
            if let Some(parent) = dest.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let _ = copy2(src, &dest);
        }

        let _ = Command::new("git")
            .arg("--git-dir")
            .arg(&self.git_dir)
            .arg("--work-tree")
            .arg(temp_baseline)
            .args(["checkout", "HEAD", "--", "."])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();

        self.prune_ignored_files(temp_baseline, &current_rel);

        let mut cmd = Command::new("git");
        cmd.args([
            "diff",
            "--no-index",
            "temp_diff_baseline",
            "temp_diff_current",
        ])
        .current_dir(codigest);
        let Some((stdout, _)) = text_output(&mut cmd) else {
            return String::new();
        };
        let diff_text = stdout
            .replace("temp_diff_baseline/", "")
            .replace("temp_diff_current/", "");

        let mut clean = Vec::new();
        let mut skip = false;
        for line in splitlines(&diff_text) {
            if line.starts_with("diff --git") {
                skip = is_internal(line);
            }
            if !skip {
                clean.push(line);
            }
        }
        clean.join("\n")
    }

    /// Drops baseline files that are still in the project but were not scanned (newly
    /// ignored, or outside the filters), so they are not reported as deleted. A file that
    /// is gone from disk stays and shows as deleted. Judged by the path inside the
    /// baseline: the absolute one always runs through `.codigest`.
    fn prune_ignored_files(&self, baseline_dir: &Path, valid: &BTreeSet<PathBuf>) {
        for file in walk(baseline_dir) {
            if file.is_file()
                && let Ok(rel) = file.strip_prefix(baseline_dir)
                && !INTERNAL_DIRS.iter().any(|dir| has_component(rel, dir))
                && !valid.contains(rel)
                && self.root.join(rel).exists()
            {
                let _ = fs::remove_file(&file);
            }
        }
    }

    /// "Never", "Unknown", or git's relative date of the last snapshot ("3 minutes ago").
    pub fn last_update_time(&self) -> String {
        if !self.git_dir.exists() {
            return "Never".to_string();
        }
        let _ = fs::create_dir_all(&self.anchor_dir);
        let mut cmd = self.git(None);
        cmd.args(["log", "-1", "--format=%cr"]);
        match text_output(&mut cmd) {
            Some((out, _)) => trim(&out),
            None => "Unknown".to_string(),
        }
    }

    /// A file as it was at the last anchor, or "" if it was not there.
    pub fn read_anchor_file(&self, rel_path: &Path) -> String {
        if !self.git_dir.exists() {
            return String::new();
        }
        let git_path = rel_path
            .components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        let mut cmd = Command::new("git");
        cmd.arg("--git-dir")
            .arg(&self.git_dir)
            .arg("--work-tree")
            .arg(&self.anchor_dir)
            .args(["show", &format!("HEAD:{git_path}")]);
        match text_output(&mut cmd) {
            Some((out, true)) => out,
            _ => String::new(),
        }
    }

    /// Files named in `get_changes`, in path order.
    pub fn get_changed_files(&self, current_files: &[PathBuf]) -> anyhow::Result<Vec<PathBuf>> {
        let raw = self.get_changes(current_files)?;
        let mut paths: BTreeSet<PathBuf> = BTreeSet::new();
        for line in splitlines(&raw) {
            if let Some((_, new)) = diff_header_paths(line)
                && !is_internal(line)
            {
                paths.insert(self.root.join(new));
            }
        }
        let mut sorted: Vec<PathBuf> = paths.into_iter().collect();
        sorted.sort_by_key(|p| sort_key(p));
        Ok(sorted)
    }
}

// --------------------------------------------------------------------------
// Helpers
// --------------------------------------------------------------------------

/// Index of the quote that closes the quoted token `text` starts with.
fn quoted_end(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut i = 1;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b'"' => return Some(i),
            _ => i += 1,
        }
    }
    None
}

/// Undoes git's C-style quoting of a path: `"a/\303\245"` -> `a/å`.
fn unquote(token: &str) -> String {
    let body = &token.as_bytes()[1..token.len() - 1];
    let mut out: Vec<u8> = Vec::with_capacity(body.len());
    let mut i = 0;
    while i < body.len() {
        if body[i] != b'\\' || i + 1 == body.len() {
            out.push(body[i]);
            i += 1;
        } else if (b'0'..=b'7').contains(&body[i + 1]) {
            let mut j = i + 1;
            let mut value = 0u32;
            while j < body.len() && j < i + 4 && (b'0'..=b'7').contains(&body[j]) {
                value = value * 8 + u32::from(body[j] - b'0');
                j += 1;
            }
            out.push((value & 0xFF) as u8);
            i = j;
        } else {
            out.push(match body[i + 1] {
                b'a' => 7,
                b'b' => 8,
                b'f' => 12,
                b'n' => 10,
                b'r' => 13,
                b't' => 9,
                b'v' => 11,
                other => other,
            });
            i += 2;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The old and new path of a `diff --git` line, without git's `a/` and `b/` prefixes;
/// None if the line is not such a header.
///
/// A name with a space is written bare, so `a/my dir/c.md b/my dir/c.md` is split where
/// both halves read the same. A name with a quote, a tab or non-ASCII characters is written
/// in quotes with C-style escapes. If the two names differ and contain " b/" the split is
/// a guess.
pub fn diff_header_paths(line: &str) -> Option<(String, String)> {
    let rest = line.strip_prefix("diff --git ").filter(|r| !r.is_empty())?;

    let (old, new) = if rest.starts_with('"') {
        let end = quoted_end(rest)?;
        if rest.as_bytes().get(end + 1) != Some(&b' ') {
            return None;
        }
        let raw = &rest[end + 2..];
        let new = if raw.starts_with('"') {
            unquote(raw)
        } else {
            raw.to_string()
        };
        (unquote(&rest[..=end]), new)
    } else if let Some(cut) = rest.find(" \"b/") {
        (rest[..cut].to_string(), unquote(&rest[cut + 1..]))
    } else {
        let same_name = rest
            .len()
            .checked_sub(5)
            .filter(|n| n % 2 == 0)
            .and_then(|n| {
                let half = n / 2;
                let (left, right) = (rest.get(..2 + half)?, rest.get(3 + half..)?);
                let aligned = rest.as_bytes()[2 + half] == b' '
                    && left.starts_with("a/")
                    && right.starts_with("b/")
                    && left[2..] == right[2..];
                (half > 0 && aligned).then(|| (left.to_string(), right.to_string()))
            });
        match same_name {
            Some(pair) => pair,
            None if rest.starts_with("a/") => {
                let at = rest.find(" b/")?;
                (rest[..at].to_string(), rest[at + 1..].to_string())
            }
            None => return None,
        }
    };

    Some((
        old.strip_prefix("a/")?.to_string(),
        new.strip_prefix("b/")?.to_string(),
    ))
}

/// Whether a `diff --git` section is about git's or the anchor's own files, judged by whole
/// path components.
fn is_internal(header: &str) -> bool {
    diff_header_paths(header).is_some_and(|(old, new)| {
        [old, new]
            .iter()
            .any(|path| path.split('/').any(|part| INTERNAL_DIRS.contains(&part)))
    })
}

/// `Path.resolve()`: absolute and symlink-free, for a path that need not exist yet. On
/// Windows without the `\\?\` prefix that `canonicalize` adds.
pub fn resolve(path: &Path) -> PathBuf {
    let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let mut tail: Vec<&OsStr> = Vec::new();
    let mut probe: &Path = &absolute;
    loop {
        if let Ok(real) = fs::canonicalize(probe) {
            let mut out = strip_verbatim(real);
            for part in tail.iter().rev() {
                out.push(part);
            }
            return out;
        }
        match (probe.parent(), probe.file_name()) {
            (Some(parent), Some(name)) => {
                tail.push(name);
                probe = parent;
            }
            _ => return absolute,
        }
    }
}

fn strip_verbatim(path: PathBuf) -> PathBuf {
    let text = path.to_string_lossy();
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{rest}"));
    }
    match text.strip_prefix(r"\\?\") {
        Some(rest) if rest.as_bytes().get(1) == Some(&b':') => PathBuf::from(rest),
        _ => path,
    }
}

/// Files under `.git` or `.codigest` anywhere in their path are never part of an anchor.
fn skipped(path: &Path) -> bool {
    has_component(path, ".git") || has_component(path, ".codigest")
}

fn has_component(path: &Path, name: &str) -> bool {
    path.components().any(|c| c.as_os_str() == OsStr::new(name))
}

fn mtime_ns(meta: &fs::Metadata) -> i128 {
    let time = meta.modified().unwrap_or(UNIX_EPOCH);
    match time.duration_since(UNIX_EPOCH) {
        Ok(after) => i128::try_from(after.as_nanos()).unwrap_or(i128::MAX),
        Err(before) => -i128::try_from(before.duration().as_nanos()).unwrap_or(i128::MAX),
    }
}

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Every file and directory below `dir`, depth first, without following links into
/// directories (`Path.rglob("*")`).
fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let Ok(entries) = fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            // Nothing below the anchor's repository is ever wanted, and it is the
            // largest part of the tree.
            if entry.file_type().is_ok_and(|t| t.is_dir()) && entry.file_name() != SHADOW_DIR {
                stack.push(path.clone());
            }
            found.push(path);
        }
    }
    found
}

/// stdout of a finished command as `text=True` reads it: lossy UTF-8 with every line
/// ending turned into `\n`. None if the program could not be started; the flag says
/// whether it succeeded.
fn text_output(cmd: &mut Command) -> Option<(String, bool)> {
    let output = cmd
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout)
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    Some((text, output.status.success()))
}

fn trim(text: &str) -> String {
    text.trim_matches(is_space).to_string()
}

/// `str.splitlines()`.
pub fn splitlines(text: &str) -> Vec<&str> {
    let mut lines = Vec::new();
    let mut start = 0;
    let mut chars = text.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        let is_break = matches!(
            c,
            '\n' | '\r'
                | '\x0b'
                | '\x0c'
                | '\x1c'
                | '\x1d'
                | '\x1e'
                | '\u{85}'
                | '\u{2028}'
                | '\u{2029}'
        );
        if !is_break {
            continue;
        }
        lines.push(&text[start..i]);
        let mut end = i + c.len_utf8();
        if c == '\r' && chars.peek().is_some_and(|&(_, next)| next == '\n') {
            chars.next();
            end += 1;
        }
        start = end;
    }
    if start < text.len() {
        lines.push(&text[start..]);
    }
    lines
}

/// Orders paths component by component, ignoring case on Windows, like `PurePath`.
fn sort_key(path: &Path) -> Vec<String> {
    path.components()
        .map(|c| {
            let part = c.as_os_str().to_string_lossy().into_owned();
            if cfg!(windows) {
                part.to_lowercase()
            } else {
                part
            }
        })
        .collect()
}

/// `Path.write_text`: line endings follow the platform.
fn write_text(path: &Path, text: &str) -> io::Result<()> {
    if cfg!(windows) {
        fs::write(path, text.replace('\n', "\r\n"))
    } else {
        fs::write(path, text)
    }
}

/// Makes Git ignore everything in `directory`, the way pytest and ruff caches do, so a
/// project that never listed `.codigest/` in its .gitignore does not see it in `git status`.
pub fn hide_from_git(directory: &Path) -> io::Result<()> {
    fs::create_dir_all(directory)?;
    let marker = directory.join(".gitignore");
    if !marker.exists() {
        write_text(&marker, "*\n")?;
    }
    Ok(())
}

/// `shutil.rmtree` that also deletes read-only files (git objects on Windows).
pub fn remove_tree(path: &Path) -> io::Result<()> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(_) => {
            for entry in walk(path) {
                if let Ok(meta) = fs::metadata(&entry) {
                    let mut permissions = meta.permissions();
                    #[allow(clippy::permissions_set_readonly_false)]
                    permissions.set_readonly(false);
                    let _ = fs::set_permissions(&entry, permissions);
                }
            }
            fs::remove_dir_all(path)
        }
    }
}

/// `shutil.copy2`: content, permissions and timestamps.
fn copy2(src: &Path, dest: &Path) -> io::Result<()> {
    let meta = fs::metadata(src)?;
    let mut input = File::open(src)?;
    let mut output = File::create(dest)?;
    io::copy(&mut input, &mut output)?;
    let mut times = FileTimes::new();
    if let Ok(accessed) = meta.accessed() {
        times = times.set_accessed(accessed);
    }
    if let Ok(modified) = meta.modified() {
        times = times.set_modified(modified);
    }
    output.set_times(times)?;
    drop(output);
    fs::set_permissions(dest, meta.permissions())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_lines_like_python() {
        assert_eq!(splitlines("a\nb\r\nc\rd\x0ce"), ["a", "b", "c", "d", "e"]);
        assert_eq!(splitlines("a\n"), ["a"]);
        assert_eq!(splitlines("\n\nx"), ["", "", "x"]);
        assert!(splitlines("").is_empty());
        assert_eq!(splitlines("a\u{2028}b"), ["a", "b"]);
    }

    #[test]
    fn sorts_component_wise() {
        let mut paths = vec![
            PathBuf::from("a b/x"),
            PathBuf::from("a/x"),
            PathBuf::from("a/b"),
        ];
        paths.sort_by_key(|p| sort_key(p));
        assert_eq!(
            paths,
            [
                PathBuf::from("a/b"),
                PathBuf::from("a/x"),
                PathBuf::from("a b/x")
            ]
        );
    }

    #[test]
    fn resolves_paths_that_do_not_exist_yet() {
        let dir = tempfile::tempdir().unwrap();
        let real = resolve(dir.path());
        assert_eq!(
            resolve(&dir.path().join("a").join("b")),
            real.join("a").join("b")
        );
        assert!(!real.to_string_lossy().starts_with(r"\\?\"));
    }

    #[test]
    fn marks_directories_with_a_catch_all_ignore() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join(".codigest");
        hide_from_git(&target).unwrap();
        let text = fs::read_to_string(target.join(".gitignore")).unwrap();
        assert_eq!(text.replace("\r\n", "\n"), "*\n");
        fs::write(target.join(".gitignore"), "mine\n").unwrap();
        hide_from_git(&target).unwrap();
        assert_eq!(
            fs::read_to_string(target.join(".gitignore")).unwrap(),
            "mine\n"
        );
    }

    #[test]
    fn diff_headers_are_parsed_like_git_writes_them() {
        let pair = |a: &str, b: &str| Some((a.to_string(), b.to_string()));
        let cases = [
            ("diff --git a/a.py b/a.py", pair("a.py", "a.py")),
            (
                "diff --git a/my dir/c.md b/my dir/c.md",
                pair("my dir/c.md", "my dir/c.md"),
            ),
            (
                r#"diff --git "a/\355\225\234.txt" "b/\355\225\234.txt""#,
                pair("한.txt", "한.txt"),
            ),
            (
                r#"diff --git "a/tab\there" "b/tab\there""#,
                pair("tab\there", "tab\there"),
            ),
            (
                r#"diff --git "a/q\"uote" "b/q\"uote""#,
                pair("q\"uote", "q\"uote"),
            ),
            (
                r#"diff --git "a/back\\slash" "b/back\\slash""#,
                pair("back\\slash", "back\\slash"),
            ),
            (
                "diff --git a/old name.py b/new name.py",
                pair("old name.py", "new name.py"),
            ),
            (
                r#"diff --git a/plain.py "b/\355\225\234.py""#,
                pair("plain.py", "한.py"),
            ),
            ("--- a/a.py", None),
            ("diff --git a/only-one", None),
        ];
        for (line, expected) in cases {
            assert_eq!(diff_header_paths(line), expected, "{line}");
        }
    }

    #[test]
    fn internal_sections_are_judged_by_path_component() {
        for internal in [
            "diff --git a/.git/config b/.git/config",
            "diff --git a/sub/.shadow_git/HEAD b/sub/.shadow_git/HEAD",
            "diff --git a/.codigest/x b/.codigest/x",
        ] {
            assert!(is_internal(internal), "{internal}");
        }
        for ordinary in [
            "diff --git a/.gitignore b/.gitignore",
            "diff --git a/.github/ci.yml b/.github/ci.yml",
            "diff --git a/.gitattributes b/.gitattributes",
            "diff --git a/my.git.md b/my.git.md",
        ] {
            assert!(!is_internal(ordinary), "{ordinary}");
        }
    }
}
