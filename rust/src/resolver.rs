//! The project files a file imports (`core/resolver.py`): Python through its import
//! statements, JS/TS through relative specifiers. `scan -r` follows them recursively,
//! `pack` one hop.

use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;
use tree_sitter::{Node, Parser};

use crate::paths::cmp_paths;

/// `sys.stdlib_module_names` of Python 3.15.
static STDLIB: LazyLock<HashSet<&'static str>> =
    LazyLock::new(|| include_str!("stdlib_names.txt").lines().collect());

const JS_SUFFIXES: [&str; 6] = ["ts", "tsx", "js", "jsx", "mjs", "cjs"];
// Relative specifiers only: `from './x'`, `import './x'`, `require('./x')`, `import('./x')`.
static JS_IMPORT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?:\bfrom|\bimport|\brequire\(|\bimport\()\s*['"](\.{1,2}/[^'"]+)['"]"#)
        .expect("valid pattern")
});

pub struct Resolver {
    root: PathBuf,
    cache: HashMap<String, Option<PathBuf>>,
}

/// `Path.resolve()`: on Windows this also gives the on-disk spelling, so `Diffractometer.py`
/// found on a case-insensitive drive is the same file as `diffractometer.py`.
fn canonical(path: &Path) -> PathBuf {
    match fs::canonicalize(path) {
        Ok(real) => crate::paths::strip_verbatim(real),
        Err(_) => std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf()),
    }
}

impl Resolver {
    pub fn new(root: &Path) -> Self {
        Self {
            root: canonical(root),
            cache: HashMap::new(),
        }
    }

    /// Project files that `path` imports, for any supported language.
    pub fn imports_of(&mut self, path: &Path) -> HashSet<PathBuf> {
        match path.extension().and_then(|e| e.to_str()) {
            Some("py" | "pyi") => self.python_imports(path),
            Some(ext) if JS_SUFFIXES.contains(&ext) => js_imports(path),
            _ => HashSet::new(),
        }
    }

    /// `files` and everything they import, transitively, sorted like Python sorts paths.
    pub fn resolve(&mut self, files: &[PathBuf]) -> Vec<PathBuf> {
        let mut visited: HashSet<PathBuf> = files.iter().map(|f| canonical(f)).collect();
        let mut queue: VecDeque<PathBuf> = files.iter().map(|f| canonical(f)).collect();
        let mut results: Vec<PathBuf> = files.to_vec();
        while let Some(current) = queue.pop_front() {
            for dep in self.imports_of(&current) {
                if visited.insert(dep.clone()) {
                    queue.push_back(dep.clone());
                    results.push(dep);
                }
            }
        }
        results.sort_by(|a, b| cmp_paths(a, b));
        results.dedup();
        results
    }

    fn python_imports(&mut self, path: &Path) -> HashSet<PathBuf> {
        let mut found = HashSet::new();
        let Ok(code) = fs::read_to_string(path) else {
            return found;
        };
        for (module, level, names) in python_import_statements(&code) {
            if !module.is_empty()
                && let Some(p) = self.lookup(&module, level, path)
            {
                found.insert(p);
            }
            for name in names {
                let full = if module.is_empty() {
                    name
                } else {
                    format!("{module}.{name}")
                };
                if let Some(p) = self.lookup(&full, level, path) {
                    found.insert(p);
                }
            }
        }
        found
    }

    fn lookup(&mut self, module: &str, level: usize, current: &Path) -> Option<PathBuf> {
        if level == 0 && STDLIB.contains(module.split('.').next().unwrap_or("")) {
            return None;
        }
        // Relative imports depend on the importing file, so only absolute ones are cached.
        if level > 0 {
            return self.find(module, level, current);
        }
        if let Some(hit) = self.cache.get(module) {
            return hit.clone();
        }
        let result = self.find(module, level, current);
        self.cache.insert(module.to_string(), result.clone());
        result
    }

    fn find(&self, module: &str, level: usize, current: &Path) -> Option<PathBuf> {
        if module.is_empty() {
            return None;
        }
        let parts: Vec<&str> = module.split('.').collect();
        let join = |base: &Path| parts.iter().fold(base.to_path_buf(), |acc, p| acc.join(p));
        let parent = current.parent().unwrap_or(Path::new("."));
        let candidates = if level > 0 {
            let base = (1..level).fold(parent.to_path_buf(), |dir, _| {
                dir.parent().map_or(dir.clone(), Path::to_path_buf)
            });
            vec![join(&base)]
        } else {
            vec![join(parent), join(&self.root), join(&self.root.join("src"))]
        };
        for base in candidates {
            let module_file = base.with_extension("py");
            let package = base.join("__init__.py");
            for p in [module_file, package] {
                if p.is_file() {
                    return Some(canonical(&p));
                }
            }
        }
        None
    }
}

/// (module, level, imported names) of every import statement, as `ast.walk` finds them:
/// anywhere in the file, including inside functions.
pub fn python_import_statements(code: &str) -> Vec<(String, usize, Vec<String>)> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_python::LANGUAGE.into())
        .expect("python grammar");
    let Some(tree) = parser.parse(code, None) else {
        return Vec::new();
    };
    let src = code.as_bytes();
    let text = |n: Node| n.utf8_text(src).unwrap_or("").to_string();
    let mut out = Vec::new();
    let mut stack = vec![tree.root_node()];
    while let Some(node) = stack.pop() {
        match node.kind() {
            "import_statement" => {
                let mut cursor = node.walk();
                for name in node.children_by_field_name("name", &mut cursor) {
                    let dotted = match name.kind() {
                        "aliased_import" => name
                            .child_by_field_name("name")
                            .map(text)
                            .unwrap_or_default(),
                        _ => text(name),
                    };
                    out.push((dotted, 0, Vec::new()));
                }
            }
            "import_from_statement" => {
                let Some(module) = node.child_by_field_name("module_name") else {
                    continue;
                };
                let (level, name) = match module.kind() {
                    "relative_import" => {
                        let mut cursor = module.walk();
                        let children: Vec<Node> = module.named_children(&mut cursor).collect();
                        let dots = children
                            .iter()
                            .find(|c| c.kind() == "import_prefix")
                            .map_or(0, |p| text(*p).len());
                        let dotted = children
                            .iter()
                            .find(|c| c.kind() == "dotted_name")
                            .map(|d| text(*d))
                            .unwrap_or_default();
                        (dots, dotted)
                    }
                    _ => (0, text(module)),
                };
                let mut cursor = node.walk();
                let names: Vec<String> = node
                    .children_by_field_name("name", &mut cursor)
                    .map(|n| match n.kind() {
                        "aliased_import" => {
                            n.child_by_field_name("name").map(text).unwrap_or_default()
                        }
                        _ => text(n),
                    })
                    .collect();
                out.push((name, level, names));
            }
            _ => {
                let mut cursor = node.walk();
                stack.extend(node.children(&mut cursor));
            }
        }
    }
    out
}

/// Files a JS/TS module imports by relative path, resolved the way bundlers and tsc do.
pub fn js_imports(path: &Path) -> HashSet<PathBuf> {
    let mut found = HashSet::new();
    let Ok(source) = fs::read_to_string(path) else {
        return found;
    };
    let parent = path.parent().unwrap_or(Path::new("."));
    for cap in JS_IMPORT.captures_iter(&source) {
        let base = canonical(&parent.join(&cap[1]));
        // TS ESM code imports `./x.js` for a file that is `x.ts` on disk.
        let has_js_suffix = base
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| JS_SUFFIXES.contains(&e));
        let stem = if has_js_suffix {
            base.with_extension("")
        } else {
            base.clone()
        };
        let mut candidates = vec![base.clone()];
        let stem_name = stem
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        candidates.extend(
            JS_SUFFIXES
                .iter()
                .map(|s| stem.with_file_name(format!("{stem_name}.{s}"))),
        );
        candidates.extend(JS_SUFFIXES.iter().map(|s| base.join(format!("index.{s}"))));
        if let Some(hit) = candidates.into_iter().find(|c| c.is_file()) {
            found.insert(hit);
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_imports_anywhere_including_relative_ones() {
        let code = "import os, pkg.mod as m\nfrom . import a\nfrom ..b.c import d, e as f\ndef g():\n    import inner\n";
        let mut found = python_import_statements(code);
        found.sort();
        assert_eq!(
            found,
            vec![
                ("".to_string(), 1, vec!["a".to_string()]),
                ("b.c".to_string(), 2, vec!["d".to_string(), "e".to_string()]),
                ("inner".to_string(), 0, vec![]),
                ("os".to_string(), 0, vec![]),
                ("pkg.mod".to_string(), 0, vec![]),
            ]
        );
    }

    #[test]
    fn resolves_project_modules_and_skips_the_standard_library() {
        let dir = tempfile::tempdir().unwrap();
        // CI temp folders are 8.3 short names on Windows and behind /var -> /private/var on
        // macOS; found modules come back canonical, so the root must be too for the order.
        let root = &canonical(dir.path());
        fs::create_dir_all(root.join("src/pkg")).unwrap();
        fs::write(root.join("src/pkg/__init__.py"), "from .core import f\n").unwrap();
        fs::write(root.join("src/pkg/core.py"), "import os\n").unwrap();
        fs::write(root.join("main.py"), "import pkg\nimport json\n").unwrap();
        let mut resolver = Resolver::new(root);
        let all = resolver.resolve(&[root.join("main.py")]);
        let names: Vec<String> = all
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["main.py", "__init__.py", "core.py"]);
    }
}
