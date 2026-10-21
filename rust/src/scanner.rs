//! Walks a project and returns the files to share, as `core/scanner.py` does.

use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::gitwild::GitWildSpec;
use crate::pypath;
use crate::pystr;

pub const ALWAYS_IGNORE: [&str; 11] = [
    ".git/",
    ".codigest/",
    ".venv/",
    "venv/",
    "env/",
    "__pycache__/",
    "node_modules/",
    ".idea/",
    ".vscode/",
    "*.pyc",
    "*.DS_Store",
];

/// Kept even when the extension filter would drop them.
const ALWAYS_KEEP: [&str; 3] = [".gitignore", "Dockerfile", "pyproject.toml"];

pub struct ProjectScanner {
    root: PathBuf,
    extensions: Option<HashSet<String>>,
    spec: GitWildSpec,
    include_paths: Option<Vec<PathBuf>>,
}

impl ProjectScanner {
    pub fn new(
        root: &Path,
        extensions: Option<HashSet<String>>,
        extra_ignores: &[String],
        include_paths: Option<Vec<PathBuf>>,
    ) -> Result<Self> {
        let mut patterns: Vec<String> = ALWAYS_IGNORE.iter().map(|p| (*p).to_owned()).collect();
        patterns.extend(extra_ignores.iter().cloned());
        patterns.extend(gitignore_lines(root));
        Ok(Self {
            root: root.to_path_buf(),
            // An empty set filters nothing, as in the original.
            extensions: extensions.filter(|set| !set.is_empty()),
            spec: GitWildSpec::from_lines(patterns)?,
            include_paths: include_paths.filter(|paths| !paths.is_empty()),
        })
    }

    pub fn is_ignored(&self, path: &Path) -> bool {
        let Some(rel) = pypath::rel_posix(path, &self.root) else {
            return false;
        };
        self.ignored_rel(&rel, path.is_dir())
    }

    fn ignored_rel(&self, rel: &str, is_dir: bool) -> bool {
        if is_dir {
            self.spec.match_file(&format!("{rel}/"))
        } else {
            self.spec.match_file(rel)
        }
    }

    pub fn scan(&self) -> Result<Vec<PathBuf>> {
        let mut found = Vec::new();
        // Each folder travels with its path relative to the root, so nothing below has to
        // work it out again from the ancestors.
        let mut stack: Vec<(PathBuf, String)> = Vec::new();
        let starts = self
            .include_paths
            .clone()
            .unwrap_or_else(|| vec![self.root.clone()]);
        for start in starts {
            if start.is_file() {
                if !self.is_ignored(&start) {
                    found.push(start);
                }
            } else if start.is_dir() {
                let rel = pypath::rel_posix(&start, &self.root).unwrap_or_default();
                stack.push((start, if rel == "." { String::new() } else { rel }));
            }
        }

        while let Some((current, current_rel)) = stack.pop() {
            let mut entries = match read_entries(&current) {
                Ok(entries) => entries,
                Err(err) if err.kind() == io::ErrorKind::PermissionDenied => continue,
                Err(err) => return Err(err.into()),
            };
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            for (name, kind) in entries {
                let entry = current.join(&name);
                let (is_dir, is_file) = kind.resolve(&entry);
                let rel = if current_rel.is_empty() {
                    name.clone()
                } else {
                    format!("{current_rel}/{name}")
                };
                if self.ignored_rel(&rel, is_dir) {
                    continue;
                }
                if is_dir {
                    stack.push((entry, rel));
                } else if is_file && self.keeps(&name) {
                    found.push(entry);
                }
            }
        }
        Ok(pypath::sorted_unique(found))
    }

    fn keeps(&self, name: &str) -> bool {
        let Some(extensions) = &self.extensions else {
            return true;
        };
        extensions.contains(&pypath::suffix(name).to_lowercase()) || ALWAYS_KEEP.contains(&name)
    }
}

pub fn scan_project(
    root: &Path,
    extensions: Option<HashSet<String>>,
    extra_ignores: &[String],
    include_paths: Option<Vec<PathBuf>>,
) -> Result<Vec<PathBuf>> {
    ProjectScanner::new(root, extensions, extra_ignores, include_paths)?.scan()
}

/// What a directory entry is, from the listing itself where the platform says so: a stat
/// per entry is the expensive part of walking a large tree on Windows.
#[derive(Clone, Copy)]
pub enum Kind {
    Dir,
    File,
    /// A link or anything unusual: ask the file system, following links like Python does.
    Other,
}

impl Kind {
    pub fn resolve(self, path: &Path) -> (bool, bool) {
        match self {
            Kind::Dir => (true, false),
            Kind::File => (false, true),
            Kind::Other => (path.is_dir(), path.is_file()),
        }
    }
}

pub fn read_entries(dir: &Path) -> io::Result<Vec<(String, Kind)>> {
    fs::read_dir(dir)?
        .map(|entry| {
            entry.map(|e| {
                let kind = match e.file_type() {
                    Ok(t) if t.is_dir() => Kind::Dir,
                    Ok(t) if t.is_file() => Kind::File,
                    _ => Kind::Other,
                };
                (e.file_name().to_string_lossy().into_owned(), kind)
            })
        })
        .collect()
}

/// The root `.gitignore` as pathspec is fed it: stripped lines without comments. A file that
/// cannot be read is skipped with a warning.
fn gitignore_lines(root: &Path) -> Vec<String> {
    let path = root.join(".gitignore");
    if !path.exists() {
        return Vec::new();
    }
    let text = match fs::read(&path)
        .map_err(|e| e.to_string())
        .and_then(|bytes| String::from_utf8(bytes).map_err(|e| e.to_string()))
    {
        Ok(text) => text,
        Err(why) => {
            eprintln!("Failed to read .gitignore: {why}");
            return Vec::new();
        }
    };
    pystr::universal_newlines(&text)
        .split('\n')
        .filter(|line| !line.starts_with('#'))
        .map(|line| pystr::trim(line).to_owned())
        .filter(|line| !line.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn project(files: &[&str]) -> TempDir {
        let dir = tempfile::tempdir().unwrap();
        for file in files {
            let path = dir.path().join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "x").unwrap();
        }
        dir
    }

    fn names(root: &Path, files: Vec<PathBuf>) -> Vec<String> {
        files
            .iter()
            .map(|f| pypath::rel_posix(f, root).unwrap())
            .collect()
    }

    #[test]
    fn skips_the_always_ignored_folders_and_files() {
        let dir = project(&[
            "a.py",
            ".git/config",
            "node_modules/x.js",
            "m.pyc",
            "src/b.py",
        ]);
        let root = pypath::resolve(dir.path());
        let files = scan_project(&root, None, &[], None).unwrap();
        assert_eq!(names(&root, files), ["a.py", "src/b.py"]);
    }

    #[test]
    fn the_root_gitignore_applies_with_negation() {
        let dir = project(&["a.log", "keep.log", "out/x.txt", "b.py"]);
        fs::write(
            dir.path().join(".gitignore"),
            "# c\n*.log\n!keep.log\nout/\n",
        )
        .unwrap();
        let root = pypath::resolve(dir.path());
        let files = scan_project(&root, None, &[], None).unwrap();
        assert_eq!(names(&root, files), [".gitignore", "b.py", "keep.log"]);
    }

    #[test]
    fn extension_filter_keeps_the_named_config_files() {
        let dir = project(&[
            "a.py",
            "b.TXT",
            "Dockerfile",
            "pyproject.toml",
            "c.md",
            ".gitignore",
        ]);
        let root = pypath::resolve(dir.path());
        let exts: HashSet<String> = [".py".to_owned(), ".txt".to_owned()].into();
        let files = scan_project(&root, Some(exts), &[], None).unwrap();
        // Order is case-insensitive only on Windows; this test is about which files stay.
        let mut kept = names(&root, files);
        kept.sort_by_key(|n| n.to_lowercase());
        assert_eq!(
            kept,
            [
                ".gitignore",
                "a.py",
                "b.TXT",
                "Dockerfile",
                "pyproject.toml"
            ]
        );
    }

    #[test]
    fn config_excludes_come_before_the_gitignore() {
        let dir = project(&["a.py", "gen/x.py"]);
        let root = pypath::resolve(dir.path());
        let files = scan_project(&root, None, &["gen/".to_owned()], None).unwrap();
        assert_eq!(names(&root, files), ["a.py"]);
    }

    #[test]
    fn include_paths_limit_the_walk_and_list_files_as_given() {
        let dir = project(&["a.py", "src/b.py", "src/c.txt", "lib/d.py"]);
        let root = pypath::resolve(dir.path());
        let only = vec![root.join("src"), root.join("a.py")];
        let files = scan_project(&root, None, &[], Some(only)).unwrap();
        assert_eq!(names(&root, files), ["a.py", "src/b.py", "src/c.txt"]);
    }

    #[test]
    fn ordering_goes_part_by_part() {
        let dir = project(&["a.py", "a/z.py", "b.py"]);
        let root = pypath::resolve(dir.path());
        let files = scan_project(&root, None, &[], None).unwrap();
        assert_eq!(names(&root, files), ["a/z.py", "a.py", "b.py"]);
    }
}
