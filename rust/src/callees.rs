//! The project code a failing test exercises (`core/callees.py`).
//!
//! A traceback that ends in an assertion names the test, not the function under test.
//! When that function is unchanged (the bug is committed, or the baseline moved past it),
//! nothing else in a pack shows it. The calls made by the failing test functions, matched
//! against the definitions in the project modules the test imports, recover it.

use std::fs;
use std::path::{Path, PathBuf};

use tree_sitter::{Node, Parser, Tree};

pub const MAX_DEFINITIONS: usize = 8;

type Span = (usize, usize);
type Definitions = Vec<(String, Span)>;

fn parse(code: &str) -> Option<Tree> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_python::LANGUAGE.into())
        .expect("python grammar");
    parser.parse(code, None)
}

fn children(node: Node<'_>) -> Vec<Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).collect()
}

/// Start (including decorators) and end line of a definition, 1-based.
fn span(def: Node, outer: Node) -> (usize, usize) {
    (outer.start_position().row + 1, def.end_position().row + 1)
}

/// Function definitions that contain any of `lines`, at any depth, like `ast.walk`.
fn enclosing_functions<'t>(root: Node<'t>, lines: &[usize]) -> Vec<Node<'t>> {
    let mut found = Vec::new();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if node.kind() == "function_definition" {
            let (lo, hi) = (node.start_position().row + 1, node.end_position().row + 1);
            if lines.iter().any(|l| (lo..=hi).contains(l)) {
                found.push(node);
            }
        }
        let mut cursor = node.walk();
        stack.extend(node.children(&mut cursor));
    }
    found.sort_by_key(|n| n.start_byte());
    found
}

/// Names called inside `nodes`, in order of first appearance: `f(...)` and `obj.f(...)`.
fn called_names(nodes: &[Node], src: &[u8]) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for node in nodes {
        let mut calls = Vec::new();
        let mut stack = vec![*node];
        while let Some(n) = stack.pop() {
            if n.kind() == "call" {
                calls.push(n);
            }
            let mut cursor = n.walk();
            let kids: Vec<Node> = n.children(&mut cursor).collect();
            stack.extend(kids.into_iter().rev());
        }
        calls.sort_by_key(|c| c.start_byte());
        for call in calls {
            let Some(func) = call.child_by_field_name("function") else {
                continue;
            };
            let name = match func.kind() {
                "identifier" => func.utf8_text(src).ok(),
                "attribute" => func
                    .child_by_field_name("attribute")
                    .and_then(|a| a.utf8_text(src).ok()),
                _ => None,
            };
            if let Some(name) = name
                && !names.iter().any(|n| n == name)
            {
                names.push(name.to_string());
            }
        }
    }
    names
}

/// Top-level functions and classes, and methods by their bare name, to their line span.
fn definitions(tree: &Tree, src: &[u8]) -> Vec<(String, (usize, usize))> {
    let mut spans: Vec<(String, (usize, usize))> = Vec::new();
    let mut add = |name: &str, s: (usize, usize), first_wins: bool| match spans
        .iter_mut()
        .find(|(n, _)| n == name)
    {
        Some(existing) if !first_wins => existing.1 = s,
        Some(_) => {}
        None => spans.push((name.to_string(), s)),
    };
    for stmt in children(tree.root_node()) {
        let (def, outer) = match stmt.kind() {
            "decorated_definition" => match stmt.child_by_field_name("definition") {
                Some(d) => (d, stmt),
                None => continue,
            },
            _ => (stmt, stmt),
        };
        if !matches!(def.kind(), "function_definition" | "class_definition") {
            continue;
        }
        let Some(name) = def
            .child_by_field_name("name")
            .and_then(|n| n.utf8_text(src).ok())
        else {
            continue;
        };
        add(name, span(def, outer), false);
        if def.kind() == "class_definition"
            && let Some(body) = def.child_by_field_name("body")
        {
            for item in children(body) {
                let (method, outer) = match item.kind() {
                    "decorated_definition" => match item.child_by_field_name("definition") {
                        Some(d) => (d, item),
                        None => continue,
                    },
                    _ => (item, item),
                };
                if method.kind() != "function_definition" {
                    continue;
                }
                if let Some(name) = method
                    .child_by_field_name("name")
                    .and_then(|n| n.utf8_text(src).ok())
                {
                    add(name, span(method, outer), true);
                }
            }
        }
    }
    spans
}

/// (file, name, line span) of definitions in `modules` called by the test functions that
/// contain `lines`, in call order, at most MAX_DEFINITIONS.
pub fn called_definitions(
    test: &Path,
    lines: &[usize],
    modules: &[PathBuf],
) -> Vec<(PathBuf, String, (usize, usize))> {
    let Ok(code) = fs::read_to_string(test) else {
        return Vec::new();
    };
    let Some(tree) = parse(&code) else {
        return Vec::new();
    };
    let called = called_names(
        &enclosing_functions(tree.root_node(), lines),
        code.as_bytes(),
    );

    let mut sorted = modules.to_vec();
    sorted.sort_by(|a, b| crate::paths::cmp_paths(a, b));
    let defined: Vec<(PathBuf, Definitions)> = sorted
        .into_iter()
        .filter_map(|path| {
            let code = fs::read_to_string(&path).ok()?;
            let tree = parse(&code)?;
            let defs = definitions(&tree, code.as_bytes());
            Some((path, defs))
        })
        .collect();

    let mut found = Vec::new();
    for name in called {
        if let Some((path, s)) = defined.iter().find_map(|(path, defs)| {
            defs.iter()
                .find(|(n, _)| *n == name)
                .map(|(_, s)| (path, *s))
        }) {
            found.push((path.clone(), name, s));
        }
        if found.len() == MAX_DEFINITIONS {
            break;
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_function_a_failing_test_calls() {
        let dir = tempfile::tempdir().unwrap();
        let module = dir.path().join("prices.py");
        let test = dir.path().join("test_prices.py");
        fs::write(
            &module,
            "def unrelated():\n    pass\n\n\ndef total(items):\n    return sum(items) * 2\n",
        )
        .unwrap();
        fs::write(
            &test,
            "from prices import total\n\n\ndef test_total():\n    assert total([1, 2]) == 3\n",
        )
        .unwrap();
        let found = called_definitions(&test, &[5], std::slice::from_ref(&module));
        assert_eq!(found, vec![(module, "total".to_string(), (5, 6))]);
    }
}
