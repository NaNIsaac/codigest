//! The parts of Python's `pathlib` that decide what the output looks like: equality and
//! ordering fold case on Windows (so `sorted()` lists `a.py` before `B.py` there), `resolve`
//! hands back a plain path, and `suffix` has its own idea of where an extension starts.

use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Result, bail};

/// `str(path)` after the case folding `PurePath` applies before it compares or hashes.
pub fn norm(path: &Path) -> String {
    let text = path.to_string_lossy();
    if cfg!(windows) {
        text.replace('/', "\\").to_lowercase()
    } else {
        text.into_owned()
    }
}

pub fn same(a: &Path, b: &Path) -> bool {
    norm(a) == norm(b)
}

/// `sorted(set(paths))`. Of paths that compare equal the first one stays.
pub fn sorted_unique(mut paths: Vec<PathBuf>) -> Vec<PathBuf> {
    paths.sort_by(|a, b| crate::paths::cmp_paths(a, b));
    paths.dedup_by(|b, a| same(a, b));
    paths
}

/// `path.is_relative_to(base)`: `base` is the path itself or one of its parents.
pub fn is_relative_to(path: &Path, base: &Path) -> bool {
    path.ancestors().any(|a| same(a, base))
}

/// `path.relative_to(base).as_posix()`, or `None` when `path` is outside `base`.
pub fn rel_posix(path: &Path, base: &Path) -> Option<String> {
    let ancestor = path.ancestors().find(|a| same(a, base))?;
    let rest = path.strip_prefix(ancestor).ok()?;
    let parts: Vec<String> = rest
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    Some(if parts.is_empty() {
        ".".to_string()
    } else {
        parts.join("/")
    })
}

/// `path.name`.
pub fn name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// `path.suffix` as of Python 3.15: leading dots belong to the stem, and a trailing dot is
/// a suffix of its own.
pub fn suffix(name: &str) -> &str {
    let stem = name.trim_start_matches('.');
    stem.rfind('.').map_or("", |i| &stem[i..])
}

/// `Path.resolve()`: absolute, symlinks followed, and for a path that does not exist the
/// part that does is resolved and the rest appended.
pub fn resolve(path: &Path) -> PathBuf {
    let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    if let Ok(real) = fs::canonicalize(&absolute) {
        return crate::paths::strip_verbatim(real);
    }
    let mut head = absolute;
    let mut tail: Vec<OsString> = Vec::new();
    while !head.exists() {
        let Some(name) = head.file_name().map(OsString::from) else {
            break;
        };
        tail.push(name);
        if !head.pop() {
            break;
        }
    }
    let mut out = fs::canonicalize(&head).map_or(head, crate::paths::strip_verbatim);
    out.extend(tail.into_iter().rev());
    out
}

pub fn cwd() -> Result<PathBuf> {
    Ok(resolve(&env::current_dir()?))
}

/// `os.path.commonpath`. The result is spelled like the first path.
pub fn common_path(paths: &[PathBuf]) -> Result<PathBuf> {
    fn split(path: &Path) -> (String, Vec<String>) {
        let mut head = PathBuf::new();
        let mut tail = Vec::new();
        for component in path.components() {
            match component {
                Component::Prefix(_) | Component::RootDir => head.push(component),
                Component::Normal(part) => tail.push(part.to_string_lossy().into_owned()),
                Component::CurDir | Component::ParentDir => {}
            }
        }
        (head.to_string_lossy().into_owned(), tail)
    }
    let fold = |s: &str| {
        if cfg!(windows) {
            s.to_lowercase()
        } else {
            s.to_owned()
        }
    };
    let Some(first) = paths.first() else {
        bail!("commonpath() arg is an empty sequence");
    };
    let (head, tail) = split(first);
    let mut shared = tail.len();
    for other in &paths[1..] {
        let (other_head, other_tail) = split(other);
        if fold(&other_head) != fold(&head) {
            bail!("Paths don't have the same drive");
        }
        shared = tail
            .iter()
            .zip(&other_tail)
            .take(shared)
            .take_while(|(a, b)| fold(a) == fold(b))
            .count();
    }
    let mut out = PathBuf::from(head);
    out.extend(&tail[..shared]);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suffix_follows_python_315() {
        assert_eq!(suffix("a.py"), ".py");
        assert_eq!(suffix(".gitignore"), "");
        assert_eq!(suffix("..a.b"), ".b");
        assert_eq!(suffix("a."), ".");
        assert_eq!(suffix("archive.tar.gz"), ".gz");
        assert_eq!(suffix("Makefile"), "");
    }

    #[test]
    fn relative_paths_use_slashes() {
        let base = Path::new("/p/proj");
        assert_eq!(
            rel_posix(Path::new("/p/proj/src/a.py"), base).as_deref(),
            Some("src/a.py")
        );
        assert_eq!(rel_posix(Path::new("/p/other/a.py"), base), None);
        assert!(is_relative_to(Path::new("/p/proj/x"), base));
        assert!(!is_relative_to(Path::new("/p/projx"), base));
    }

    #[test]
    fn ordering_compares_parts_not_the_joined_string() {
        // "a/b" sorts before "a.py" because the part "a" sorts before "a.py".
        let paths = vec![
            PathBuf::from("/r/a.py"),
            PathBuf::from("/r/a/b.py"),
            PathBuf::from("/r/a/b.py"),
        ];
        let sorted = sorted_unique(paths);
        assert_eq!(
            sorted,
            [PathBuf::from("/r/a/b.py"), PathBuf::from("/r/a.py")]
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_ordering_ignores_case() {
        let sorted = sorted_unique(vec![
            PathBuf::from(r"C:\r\B.py"),
            PathBuf::from(r"C:\r\a.py"),
        ]);
        assert_eq!(sorted[0], PathBuf::from(r"C:\r\a.py"));
    }

    #[test]
    fn common_path_keeps_the_shared_parts() {
        let paths = [PathBuf::from("/r/a/x.py"), PathBuf::from("/r/b/y.py")];
        assert_eq!(common_path(&paths).unwrap(), PathBuf::from("/r"));
    }
}
