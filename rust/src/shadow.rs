//! The context anchor (`core/shadow.py`): a private Git repository under
//! `<root>/.codigest/anchor/.shadow_git` that holds a copy of the project as of the last
//! `cdg mark`, so `diff`, `semdiff` and `pack` can compare against it. Git itself does the
//! storing and the diffing, through the `git` CLI, so an anchor made by the Python build
//! is read by this one and the other way round.
//!
//! The behaviour is ported as it is, including a few quirks that decide what a diff
//! contains (see the notes on `get_changes`).

use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fs::{self, File, FileTimes};
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::term::is_space;

const SHADOW_DIR: &str = ".shadow_git";
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
    ///
    /// Two behaviours are kept from the Python build because they decide what a diff
    /// holds: any hunk whose `diff --git` line contains `.git`, `.codigest` or
    /// `.shadow_git` is dropped (so `.gitignore`, `.github/...` and `.gitattributes`
    /// never appear), and files that are in the anchor but outside the scan show up as
    /// deleted, because the pruning step only looks at paths that contain no `.codigest`
    /// and the scratch directories live inside `.codigest`.
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
                skip = [SHADOW_DIR, ".codigest", ".git"]
                    .iter()
                    .any(|x| line.contains(x));
            }
            if !skip {
                clean.push(line);
            }
        }
        clean.join("\n")
    }

    /// Drops files from the baseline copy that exist in the project but were not scanned.
    /// Ported literally: the path being tested is the absolute one, which always contains
    /// `.codigest`, so in practice nothing is removed.
    fn prune_ignored_files(&self, baseline_dir: &Path, valid: &BTreeSet<PathBuf>) {
        for file in walk(baseline_dir) {
            if file.is_file()
                && !has_component(&file, ".git")
                && !has_component(&file, ".codigest")
                && !has_component(&file, SHADOW_DIR)
                && let Ok(rel) = file.strip_prefix(baseline_dir)
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
            if !line.starts_with("diff --git") {
                continue;
            }
            let parts: Vec<&str> = line.split(is_space).filter(|p| !p.is_empty()).collect();
            if parts.len() < 4 {
                continue;
            }
            let last = parts[parts.len() - 1];
            let p = last
                .strip_prefix("b/")
                .or_else(|| last.strip_prefix("a/"))
                .unwrap_or(last);
            let path = Path::new(p);
            let excluded = path.components().any(|c| {
                [
                    OsStr::new(".codigest"),
                    OsStr::new(".git"),
                    OsStr::new(SHADOW_DIR),
                ]
                .contains(&c.as_os_str())
            });
            if !excluded {
                paths.insert(self.root.join(p));
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
}
