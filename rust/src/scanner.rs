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
        let Some(mut rel) = pypath::rel_posix(path, &self.root) else {
            return false;
        };
        if path.is_dir() {
            rel.push('/');
        }
        self.spec.match_file(&rel)
    }

    pub fn scan(&self) -> Result<Vec<PathBuf>> {
        let mut found = Vec::new();
        let mut stack = Vec::new();
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
                stack.push(start);
            }
        }

        while let Some(current) = stack.pop() {
            let mut names = match read_names(&current) {
                Ok(names) => names,
                Err(err) if err.kind() == io::ErrorKind::PermissionDenied => continue,
                Err(err) => return Err(err.into()),
            };
            names.sort();
            for name in names {
                let entry = current.join(&name);
                if self.is_ignored(&entry) {
                    continue;
                }
                if entry.is_dir() {
                    stack.push(entry);
                } else if entry.is_file() && self.keeps(&name) {
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

fn read_names(dir: &Path) -> io::Result<Vec<String>> {
    fs::read_dir(dir)?
        .map(|entry| entry.map(|e| e.file_name().to_string_lossy().into_owned()))
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
        assert_eq!(
            names(&root, files),
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
