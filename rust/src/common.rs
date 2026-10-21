//! Project discovery and the file list every command starts from, as `core/common.py`.

use std::collections::BTreeSet;
use std::fs;
use std::io::{self, BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::pypath;
use crate::redact::is_secret_file;
use crate::scanner;
use crate::style::yellow;

pub struct ProjectContext {
    pub start_path: PathBuf,
    pub root_path: PathBuf,
    /// Credential files met so far, so commands can say they were left out.
    pub withheld: Vec<PathBuf>,
    pub config_extensions: Option<BTreeSet<String>>,
    pub config_ignores: Vec<String>,
}

impl ProjectContext {
    /// Paths in different folders of the same project share the project you are in;
    /// starting from the first path alone would make its folder the root.
    pub fn new(targets: &[PathBuf]) -> Result<Self> {
        let cwd = pypath::cwd()?;
        let resolved: Vec<PathBuf> = targets.iter().map(|t| pypath::resolve(t)).collect();
        let start_path = if resolved.iter().all(|t| pypath::is_relative_to(t, &cwd)) {
            cwd
        } else {
            pypath::common_path(&resolved)?
        };
        let root_path = find_project_root(&start_path);
        let (config_extensions, config_ignores) = load_config_filters(&root_path);
        Ok(Self {
            start_path,
            root_path,
            withheld: Vec::new(),
            config_extensions,
            config_ignores,
        })
    }

    /// Every file under `targets` (the whole project when empty) that passes the filters.
    pub fn get_target_files(
        &mut self,
        targets: &[PathBuf],
        ignore_config: bool,
    ) -> Result<Vec<PathBuf>> {
        let scope: Vec<PathBuf> = targets.iter().map(|t| pypath::resolve(t)).collect();
        let (extensions, ignores) = if ignore_config {
            (None, Vec::new())
        } else {
            (
                self.config_extensions
                    .as_ref()
                    .map(|set| set.iter().cloned().collect()),
                self.config_ignores.clone(),
            )
        };

        for path in &scope {
            if !pypath::is_relative_to(path, &self.root_path) {
                eprintln!(
                    "{}",
                    yellow(&format!(
                        "[Warning] {} is outside detected root {}",
                        pypath::name(path),
                        self.root_path.display()
                    ))
                );
            }
        }

        let files = scanner::scan_project(
            &self.root_path,
            extensions,
            &ignores,
            Some(scope).filter(|s| !s.is_empty()),
        )?;

        let secret = |f: &PathBuf| is_secret_file(&pypath::name(f));
        let mut withheld: Vec<PathBuf> = files.iter().filter(|f| secret(f)).cloned().collect();
        withheld.append(&mut self.withheld);
        self.withheld = pypath::sorted_unique(withheld);
        Ok(files.into_iter().filter(|f| !secret(f)).collect())
    }
}

/// The nearest folder up the tree holding `.codigest`, `.git` or `.shadow_git`.
fn find_project_root(start: &Path) -> PathBuf {
    let current = if start.is_dir() {
        start
    } else {
        start.parent().unwrap_or(start)
    };
    for parent in current.ancestors() {
        if parent.join(".codigest").exists()
            || parent.join(".git").exists()
            || parent.join(".shadow_git").exists()
        {
            return parent.to_path_buf();
        }
    }
    current.to_path_buf()
}

/// `[filter]` of `.codigest/config.toml`. A file that cannot be read or parsed filters
/// nothing, and so does an `extensions` value that is not a list.
fn load_config_filters(root: &Path) -> (Option<BTreeSet<String>>, Vec<String>) {
    let path = root.join(".codigest").join("config.toml");
    let Some(table) = fs::read_to_string(path)
        .ok()
        .and_then(|text| text.parse::<toml::Table>().ok())
    else {
        return (None, Vec::new());
    };
    let Some(filters) = table.get("filter").and_then(toml::Value::as_table) else {
        return (None, Vec::new());
    };

    let extensions = match filters.get("extensions") {
        None => None,
        Some(value) => match iterate(value) {
            Some(items) => Some(items).filter(|items| !items.is_empty()),
            // `set(5)` raises in the original, which drops the whole section.
            None => return (None, Vec::new()),
        },
    };
    let ignores = filters
        .get("exclude_patterns")
        .and_then(iterate)
        .map(|items| items.into_iter().collect())
        .unwrap_or_default();
    (extensions.map(|items| items.into_iter().collect()), ignores)
}

/// What Python's `iter()` yields for a TOML value: list items, the characters of a string,
/// the keys of a table. Items that are not strings are skipped; they match nothing.
fn iterate(value: &toml::Value) -> Option<Vec<String>> {
    match value {
        toml::Value::Array(items) => Some(
            items
                .iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect(),
        ),
        toml::Value::String(text) => Some(text.chars().map(String::from).collect()),
        toml::Value::Table(table) => Some(table.keys().cloned().collect()),
        _ => None,
    }
}

/// `shutil.rmtree` that also deletes read-only files (git objects on Windows).
pub fn remove_tree(path: &Path) -> io::Result<()> {
    fs::remove_dir_all(path).or_else(|_| {
        make_writable(path);
        fs::remove_dir_all(path)
    })
}

fn make_writable(path: &Path) {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return;
    };
    let mut permissions = meta.permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    permissions.set_readonly(false);
    let _ = fs::set_permissions(path, permissions);
    if meta.is_dir()
        && let Ok(entries) = fs::read_dir(path)
    {
        for entry in entries.flatten() {
            make_writable(&entry.path());
        }
    }
}

/// Make Git ignore everything in `directory` (the way pytest and ruff caches do), so a
/// project that never listed `.codigest/` in its .gitignore does not see it in `git status`.
pub fn hide_from_git(directory: &Path) -> io::Result<()> {
    fs::create_dir_all(directory)?;
    let marker = directory.join(".gitignore");
    if !marker.exists() {
        fs::write(marker, "*\n")?;
    }
    Ok(())
}

/// `typer.confirm`: asks on stdout, default no. `None` when there is no answer to read.
pub fn confirm(question: &str) -> Option<bool> {
    let stdin = io::stdin();
    loop {
        print!("{question} [y/N]: ");
        let _ = io::stdout().flush();
        let mut line = String::new();
        if stdin.lock().read_line(&mut line).ok()? == 0 {
            return None;
        }
        match line.trim().to_lowercase().as_str() {
            "y" | "yes" => return Some(true),
            "n" | "no" | "" => return Some(false),
            _ => println!("Error: invalid input"),
        }
    }
}

pub fn stdin_is_terminal() -> bool {
    io::stdin().is_terminal()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn project(config: Option<&str>) -> TempDir {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join(".git")).unwrap();
        fs::create_dir(dir.path().join("src")).unwrap();
        for file in ["a.py", "notes.txt", ".env", "src/b.py"] {
            fs::write(dir.path().join(file), "x").unwrap();
        }
        if let Some(text) = config {
            fs::create_dir(dir.path().join(".codigest")).unwrap();
            fs::write(dir.path().join(".codigest").join("config.toml"), text).unwrap();
        }
        dir
    }

    #[test]
    fn root_is_the_nearest_folder_with_a_marker() {
        let dir = project(None);
        let root = pypath::resolve(dir.path());
        assert_eq!(find_project_root(&root.join("src")), root);
        assert_eq!(find_project_root(&root.join("src").join("b.py")), root);
    }

    #[test]
    fn config_filters_come_from_the_filter_table() {
        let dir = project(Some(
            "[filter]\nextensions = [\".py\"]\nexclude_patterns = [\"src/\"]\n",
        ));
        let (exts, ignores) = load_config_filters(dir.path());
        assert_eq!(exts.unwrap().into_iter().collect::<Vec<_>>(), [".py"]);
        assert_eq!(ignores, ["src/"]);
    }

    #[test]
    fn broken_or_missing_config_filters_nothing() {
        assert_eq!(
            load_config_filters(project(None).path()),
            (None, Vec::new())
        );
        let broken = project(Some("[filter\n"));
        assert_eq!(load_config_filters(broken.path()), (None, Vec::new()));
        let empty = project(Some("[filter]\nextensions = []\n"));
        assert_eq!(load_config_filters(empty.path()).0, None);
    }

    #[test]
    fn a_string_is_read_as_its_characters() {
        let dir = project(Some("[filter]\nextensions = \".py\"\n"));
        let exts = load_config_filters(dir.path()).0.unwrap();
        assert_eq!(exts.into_iter().collect::<Vec<_>>(), [".", "p", "y"]);
    }

    #[test]
    fn secret_files_are_withheld_and_remembered() {
        let dir = project(None);
        let root = pypath::resolve(dir.path());
        let mut ctx = ProjectContext {
            start_path: root.clone(),
            root_path: root.clone(),
            withheld: Vec::new(),
            config_extensions: None,
            config_ignores: Vec::new(),
        };
        let files = ctx.get_target_files(&[], false).unwrap();
        let rels: Vec<String> = files
            .iter()
            .map(|f| pypath::rel_posix(f, &root).unwrap())
            .collect();
        assert_eq!(rels, ["a.py", "notes.txt", "src/b.py"]);
        assert_eq!(ctx.withheld, [root.join(".env")]);
    }

    #[test]
    fn hiding_writes_the_marker_once() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join(".codigest");
        hide_from_git(&target).unwrap();
        assert_eq!(
            fs::read_to_string(target.join(".gitignore")).unwrap(),
            "*\n"
        );
        fs::write(target.join(".gitignore"), "custom\n").unwrap();
        hide_from_git(&target).unwrap();
        assert_eq!(
            fs::read_to_string(target.join(".gitignore")).unwrap(),
            "custom\n"
        );
    }

    #[test]
    fn remove_tree_deletes_read_only_files() {
        let dir = tempfile::tempdir().unwrap();
        let tree = dir.path().join("t");
        fs::create_dir_all(tree.join("sub")).unwrap();
        let file = tree.join("sub").join("f");
        fs::write(&file, "x").unwrap();
        let mut perms = fs::metadata(&file).unwrap().permissions();
        perms.set_readonly(true);
        fs::set_permissions(&file, perms).unwrap();
        remove_tree(&tree).unwrap();
        assert!(!tree.exists());
    }
}
