//! Git operations, read-only, against the user's real repository (`core/git_ops.py`).

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::shadow::splitlines;

pub fn is_git_repo(root: &Path) -> bool {
    root.join(".git").exists()
}

/// stdout as `text=True` reads it, or None when git fails.
fn git(root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    output.status.success().then(|| {
        String::from_utf8_lossy(&output.stdout)
            .replace("\r\n", "\n")
            .replace('\r', "\n")
    })
}

/// `git diff HEAD` (staged and unstaged) and the untracked files, or None outside a
/// repository with commits. Untracked files come back as paths, not inlined into the diff,
/// so callers can budget them like any other file.
pub fn working_changes(root: &Path) -> Option<(String, Vec<PathBuf>)> {
    if !is_git_repo(root) {
        return None;
    }
    let diff = git(root, &["diff", "HEAD"])?;
    let untracked = git(root, &["ls-files", "--others", "--exclude-standard"]).unwrap_or_default();
    Some((
        diff,
        splitlines(&untracked)
            .into_iter()
            .map(|line| root.join(line))
            .collect(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(root: &Path, args: &[&str]) {
        let status = Command::new("git")
            .args(["-c", "user.email=t@t", "-c", "user.name=t"])
            .args(args)
            .current_dir(root)
            .stdout(Stdio::null())
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?}");
    }

    #[test]
    fn reports_the_diff_against_head_and_untracked_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        assert!(working_changes(root).is_none(), "not a repository");

        run(root, &["init", "-q"]);
        assert!(working_changes(root).is_none(), "no commits yet");

        std::fs::write(root.join("a.txt"), "one\n").unwrap();
        run(root, &["add", "."]);
        run(root, &["commit", "-q", "-m", "first"]);
        std::fs::write(root.join("a.txt"), "two\n").unwrap();
        std::fs::write(root.join("new.txt"), "n\n").unwrap();

        let (diff, untracked) = working_changes(root).unwrap();
        assert!(diff.contains("-one") && diff.contains("+two"));
        assert_eq!(untracked, [root.join("new.txt")]);
    }
}
