//! The ASCII directory tree of `core/structure.py`.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::pypath;

/// A standard tree of the given files. Below `depth` levels a folder is shown once with the
/// number of files under it.
pub fn generate_ascii_tree(paths: &[PathBuf], root: &Path, depth: Option<usize>) -> String {
    let files: HashSet<String> = paths.iter().map(|p| pypath::norm(p)).collect();
    let dirs_with_files: HashSet<String> = paths
        .iter()
        .flat_map(|p| p.ancestors().skip(1))
        .map(pypath::norm)
        .collect();
    let keys: Vec<String> = paths.iter().map(|p| pypath::norm(p)).collect();

    let walker = Walker {
        files,
        dirs_with_files,
        keys,
        depth,
    };
    let mut lines = Vec::new();
    walker.walk(root, "", 1, &mut lines);
    lines.join("\n")
}

struct Walker {
    files: HashSet<String>,
    dirs_with_files: HashSet<String>,
    keys: Vec<String>,
    depth: Option<usize>,
}

impl Walker {
    fn walk(&self, current: &Path, prefix: &str, level: usize, lines: &mut Vec<String>) {
        let Ok(entries) = fs::read_dir(current) else {
            return;
        };
        let mut names: Vec<String> = entries
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();

        // Only scanned files and folders that hold some.
        let shown: Vec<(String, PathBuf, bool)> = names
            .into_iter()
            .filter_map(|name| {
                let path = current.join(&name);
                if path.is_file() {
                    self.files
                        .contains(&pypath::norm(&path))
                        .then_some((name, path, false))
                } else if path.is_dir() {
                    self.dirs_with_files
                        .contains(&pypath::norm(&path))
                        .then_some((name, path, true))
                } else {
                    None
                }
            })
            .collect();

        let count = shown.len();
        for (i, (name, path, is_dir)) in shown.into_iter().enumerate() {
            let last = i == count - 1;
            let connector = if last { "└── " } else { "├── " };

            if is_dir && self.depth.is_some_and(|depth| level >= depth) {
                let under = self.count_under(&path);
                let plural = if under == 1 { "" } else { "s" };
                lines.push(format!("{prefix}{connector}{name}/ ({under} file{plural})"));
                continue;
            }

            lines.push(format!("{prefix}{connector}{name}"));
            if is_dir {
                let extension = if last { "    " } else { "│   " };
                self.walk(&path, &format!("{prefix}{extension}"), level + 1, lines);
            }
        }
    }

    fn count_under(&self, dir: &Path) -> usize {
        let base = pypath::norm(dir);
        let inside = format!(
            "{}{}",
            base.trim_end_matches(std::path::MAIN_SEPARATOR),
            std::path::MAIN_SEPARATOR
        );
        self.keys
            .iter()
            .filter(|key| **key == base || key.starts_with(&inside))
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(files: &[&str]) -> (tempfile::TempDir, PathBuf, Vec<PathBuf>) {
        let dir = tempfile::tempdir().unwrap();
        let root = pypath::resolve(dir.path());
        let mut paths = Vec::new();
        for file in files {
            let path = root.join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, "").unwrap();
            paths.push(path);
        }
        (dir, root, paths)
    }

    #[test]
    fn lists_files_and_their_folders_with_connectors() {
        let (_d, root, paths) = project(&["a.py", "src/b.py", "src/pkg/c.py", "z.py"]);
        assert_eq!(
            generate_ascii_tree(&paths, &root, None),
            "├── a.py\n├── src\n│   ├── b.py\n│   └── pkg\n│       └── c.py\n└── z.py"
        );
    }

    #[test]
    fn unscanned_files_and_empty_folders_are_left_out() {
        let (_d, root, paths) = project(&["a.py", "skip/x.py"]);
        fs::create_dir(root.join("empty")).unwrap();
        fs::write(root.join("b.txt"), "").unwrap();
        assert_eq!(generate_ascii_tree(&paths[..1], &root, None), "└── a.py");
    }

    #[test]
    fn sibling_with_shared_prefix_does_not_pull_in_directory() {
        let (_d, root, paths) = project(&[".gitignore", "src_old/a.py"]);
        fs::create_dir(root.join("src")).unwrap();
        assert_eq!(
            generate_ascii_tree(&paths, &root, None),
            "├── .gitignore\n└── src_old\n    └── a.py"
        );
    }

    #[test]
    fn depth_folds_deeper_folders_into_a_count() {
        let (_d, root, paths) = project(&[
            "a.py",
            "src/b.py",
            "src/pkg/c.py",
            "src/pkg/d.py",
            "one/e.py",
        ]);
        assert_eq!(
            generate_ascii_tree(&paths, &root, Some(1)),
            "├── a.py\n├── one/ (1 file)\n└── src/ (3 files)"
        );
        assert_eq!(
            generate_ascii_tree(&paths, &root, Some(2)),
            "├── a.py\n├── one\n│   └── e.py\n└── src\n    ├── b.py\n    └── pkg/ (2 files)"
        );
    }

    #[test]
    fn names_sort_by_code_point_not_case_folded() {
        let (_d, root, paths) = project(&["b.py", "Bz.py", "a.py"]);
        assert_eq!(
            generate_ascii_tree(&paths, &root, None),
            "├── Bz.py\n├── a.py\n└── b.py"
        );
    }
}
