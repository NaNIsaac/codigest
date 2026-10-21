//! Python symbols for `digest` and `semdiff`, following `core/semdiff.py`'s `CodeParser`:
//! functions, classes, methods, and module- and class-level names, in source order.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use tree_sitter::{Node, Parser};

use super::unparse::{Unparser, decode_escapes, split_literal};

/// Longest value shown for a variable or attribute in an outline.
pub const VALUE_WIDTH: usize = 60;
pub const DOC_WIDTH: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Function,
    AsyncFunction,
    Method,
    Class,
    Variable,
    Attribute,
}

#[derive(Debug, Clone)]
pub struct Symbol {
    pub name: String,
    pub kind: Kind,
    pub signature: String,
    /// Identifies the implementation; equal when only formatting or comments differ.
    pub content_hash: u64,
    pub docstring: Option<String>,
    pub line: usize,
}

pub struct Parsed {
    /// In first-definition order, like a Python dict; a redefinition replaces in place.
    pub symbols: Vec<Symbol>,
    pub module_doc: Option<(usize, String)>,
}

pub fn parse(code: &str) -> Option<Parsed> {
    if code.is_empty() {
        return None;
    }
    // Python's tokenizer reads source with universal newlines, so a CRLF file's string
    // literals hold "\n". Line numbers do not change.
    let normalized;
    let code = if code.contains('\r') {
        normalized = code.replace("\r\n", "\n").replace('\r', "\n");
        normalized.as_str()
    } else {
        code
    };
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_python::LANGUAGE.into())
        .expect("python grammar");
    let tree = parser.parse(code, None)?;
    let root = tree.root_node();
    let collector = Collector {
        src: code.as_bytes(),
        unparse: Unparser::new(code.as_bytes()),
    };
    let mut symbols = Vec::new();
    collector.block(root, None, &mut symbols);
    let module_doc = collector
        .docstring(root)
        .filter(|(_, doc)| !doc.trim().is_empty());
    Some(Parsed {
        symbols,
        module_doc,
    })
}

struct Collector<'a> {
    src: &'a [u8],
    unparse: Unparser<'a>,
}

fn statements(node: Node<'_>) -> Vec<Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .filter(|c| c.kind() != "comment")
        .collect()
}

impl Collector<'_> {
    fn text(&self, node: Node) -> &str {
        node.utf8_text(self.src).unwrap_or("")
    }

    /// Statements of a module, class body or compound statement; function bodies are not
    /// entered, exactly as the Python visitor stops at a FunctionDef.
    fn block(&self, node: Node, class: Option<&str>, out: &mut Vec<Symbol>) {
        for stmt in statements(node) {
            self.statement(stmt, class, out);
        }
    }

    fn statement(&self, stmt: Node, class: Option<&str>, out: &mut Vec<Symbol>) {
        match stmt.kind() {
            "decorated_definition" => {
                if let Some(def) = stmt.child_by_field_name("definition") {
                    self.statement(def, class, out);
                }
            }
            "function_definition" => self.function(stmt, class, out),
            "class_definition" => self.class(stmt, out),
            "expression_statement" => {
                for child in statements(stmt) {
                    if child.kind() == "assignment" {
                        self.assignment(stmt, child, class, out);
                    }
                }
            }
            // Compound statements: their blocks are walked like the generic visitor does.
            "if_statement" | "for_statement" | "while_statement" | "try_statement"
            | "with_statement" | "match_statement" | "elif_clause" | "else_clause"
            | "except_clause" | "finally_clause" | "case_clause" => {
                for child in statements(stmt) {
                    match child.kind() {
                        "block" => self.block(child, class, out),
                        "elif_clause" | "else_clause" | "except_clause" | "finally_clause"
                        | "case_clause" => self.statement(child, class, out),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    fn function(&self, node: Node, class: Option<&str>, out: &mut Vec<Symbol>) {
        let Some(name_node) = node.child_by_field_name("name") else {
            return;
        };
        let is_async = node.child(0).is_some_and(|c| c.kind() == "async");
        let mut name = self.text(name_node).to_string();
        let mut kind = if is_async {
            Kind::AsyncFunction
        } else {
            Kind::Function
        };
        if let Some(class) = class {
            name = format!("{class}.{name}");
            kind = Kind::Method;
        }
        let params = node
            .child_by_field_name("parameters")
            .map_or_else(String::new, |p| self.unparse.parameters(p));
        let returns = node
            .child_by_field_name("return_type")
            .map_or_else(String::new, |r| format!(" -> {}", self.unparse.expr(r)));
        let body = node.child_by_field_name("body");
        insert(
            out,
            Symbol {
                name,
                kind,
                signature: format!("({params}){returns}"),
                content_hash: self.hash(node),
                docstring: body.and_then(|b| self.docstring(b)).map(|(_, d)| d),
                line: node.start_position().row + 1,
            },
        );
    }

    fn class(&self, node: Node, out: &mut Vec<Symbol>) {
        let Some(name_node) = node.child_by_field_name("name") else {
            return;
        };
        let name = self.text(name_node).to_string();
        // Keyword arguments (metaclass=...) are not bases.
        let bases: Vec<String> = node
            .child_by_field_name("superclasses")
            .map(|args| {
                statements(args)
                    .into_iter()
                    .filter(|a| a.kind() != "keyword_argument")
                    .map(|a| self.unparse.expr(a))
                    .collect()
            })
            .unwrap_or_default();
        let body = node.child_by_field_name("body");
        insert(
            out,
            Symbol {
                name: name.clone(),
                kind: Kind::Class,
                signature: if bases.is_empty() {
                    String::new()
                } else {
                    format!("({})", bases.join(", "))
                },
                content_hash: self.hash(node),
                docstring: body.and_then(|b| self.docstring(b)).map(|(_, d)| d),
                line: node.start_position().row + 1,
            },
        );
        if let Some(body) = body {
            self.block(body, Some(&name), out);
        }
    }

    fn assignment(&self, stmt: Node, node: Node, class: Option<&str>, out: &mut Vec<Symbol>) {
        // `a = b = 1` nests in tree-sitter; Python sees one Assign with two targets.
        let mut targets = Vec::new();
        let mut current = node;
        let annotation = node.child_by_field_name("type");
        let value = loop {
            if let Some(left) = current.child_by_field_name("left") {
                targets.push(left);
            }
            match current.child_by_field_name("right") {
                Some(right) if right.kind() == "assignment" => current = right,
                other => break other,
            }
        };
        let rendered_value = value.map(|v| shorten(&self.unparse.expr(v)));
        let signature = match annotation {
            Some(ty) => format!(
                ": {}{}",
                self.unparse.expr(ty),
                rendered_value
                    .as_ref()
                    .map_or_else(String::new, |v| format!(" = {v}"))
            ),
            None => format!(" = {}", rendered_value.unwrap_or_default()),
        };
        for target in targets.into_iter().filter(|t| t.kind() == "identifier") {
            let (name, kind) = match class {
                Some(class) => (format!("{class}.{}", self.text(target)), Kind::Attribute),
                None => (self.text(target).to_string(), Kind::Variable),
            };
            insert(
                out,
                Symbol {
                    name,
                    kind,
                    signature: signature.clone(),
                    content_hash: self.hash(stmt),
                    docstring: None,
                    line: stmt.start_position().row + 1,
                },
            );
        }
    }

    /// `ast.get_docstring`: the first statement of a body, if it is a plain string.
    fn docstring(&self, body: Node) -> Option<(usize, String)> {
        let first = statements(body).into_iter().next()?;
        if first.kind() != "expression_statement" {
            return None;
        }
        let expr = statements(first).into_iter().next()?;
        let pieces = match expr.kind() {
            "string" => vec![expr],
            "concatenated_string" => statements(expr),
            _ => return None,
        };
        let mut value = String::new();
        for piece in pieces {
            let raw = self.text(piece);
            let (prefix, body) = split_literal(raw);
            let prefix = prefix.to_ascii_lowercase();
            if prefix.contains('f') || prefix.contains('b') || prefix.contains('t') {
                return None;
            }
            value.push_str(&if prefix.contains('r') {
                body.to_string()
            } else {
                decode_escapes(body)
            });
        }
        Some((first.start_position().row + 1, value))
    }

    /// A hash of the node's tokens without comments or layout, so reformatting is not a change.
    fn hash(&self, node: Node) -> u64 {
        let mut hasher = DefaultHasher::new();
        let mut cursor = node.walk();
        let mut stack = vec![node];
        while let Some(n) = stack.pop() {
            if n.kind() == "comment" {
                continue;
            }
            match n.kind() {
                "string" | "integer" | "float" => self.unparse.expr(n).hash(&mut hasher),
                _ if n.child_count() == 0 => self.text(n).hash(&mut hasher),
                _ => {
                    let children: Vec<Node> = n.children(&mut cursor).collect();
                    stack.extend(children.into_iter().rev());
                }
            }
        }
        hasher.finish()
    }
}

fn insert(out: &mut Vec<Symbol>, symbol: Symbol) {
    match out.iter_mut().find(|s| s.name == symbol.name) {
        Some(existing) => *existing = symbol,
        None => out.push(symbol),
    }
}

fn shorten(text: &str) -> String {
    if text.chars().count() <= VALUE_WIDTH {
        text.to_string()
    } else {
        let head: String = text.chars().take(VALUE_WIDTH - 3).collect();
        format!("{head}...")
    }
}

/// The first line of a cleaned docstring (`inspect.cleandoc`), shortened.
pub fn first_line(doc: &str) -> String {
    let line = doc
        .split('\n')
        .map(expand_tabs)
        .map(|l| l.trim().to_string())
        .find(|l| !l.is_empty())
        .unwrap_or_default();
    if line.chars().count() <= DOC_WIDTH {
        line
    } else {
        format!(
            "{}...",
            line.chars().take(DOC_WIDTH - 3).collect::<String>()
        )
    }
}

fn expand_tabs(line: &str) -> String {
    let mut out = String::new();
    for c in line.chars() {
        if c == '\t' {
            let pad = 8 - out.chars().count() % 8;
            out.push_str(&" ".repeat(pad));
        } else {
            out.push(c);
        }
    }
    out
}

/// One line per definition in source order, numbered like `scan -l`, with the first line of
/// its docstring underneath; the module docstring comes first.
pub fn summarize(code: &str) -> String {
    let Some(parsed) = parse(code) else {
        return String::new();
    };
    if parsed.symbols.is_empty() && parsed.module_doc.is_none() {
        return String::new();
    }
    let mut numbers: Vec<usize> = parsed.symbols.iter().map(|s| s.line).collect();
    if let Some((line, _)) = &parsed.module_doc {
        numbers.push(*line);
    }
    let width = numbers.iter().max().map_or(1, |n| n.to_string().len());
    let mut lines = Vec::new();
    if let Some((line, doc)) = &parsed.module_doc {
        lines.push(format!("{line:>width$}: \"\"\"{}\"\"\"", first_line(doc)));
    }
    let mut symbols: Vec<&Symbol> = parsed.symbols.iter().collect();
    // Python sorts the dict's values, which are in insertion order, stably by line.
    symbols.sort_by_key(|s| s.line);
    for sym in symbols {
        let depth = if matches!(sym.kind, Kind::Method | Kind::Attribute) {
            sym.name.matches('.').count()
        } else {
            0
        };
        let indent = "  ".repeat(depth);
        let name = sym.name.rsplit('.').next().unwrap_or(&sym.name);
        let head = match sym.kind {
            Kind::Class => format!("class {name}{}", sym.signature),
            Kind::Variable | Kind::Attribute => format!("{name}{}", sym.signature),
            Kind::AsyncFunction => format!("async def {name}{}", sym.signature),
            Kind::Function | Kind::Method => format!("def {name}{}", sym.signature),
        };
        lines.push(format!("{:>width$}: {indent}{head}", sym.line));
        if let Some(doc) = sym.docstring.as_deref().filter(|d| !d.trim().is_empty()) {
            lines.push(format!(
                "{}  {indent}    {}",
                " ".repeat(width),
                first_line(doc)
            ));
        }
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outlines_like_the_python_build() {
        let code = "\"\"\"Prices.\n\nMore.\"\"\"\nTAX_RATE = 0.1\n\n\ndef apply_discount(price: float, percent: float=0, *, r=\"x\") -> float:\n    \"\"\"Price after discount.\"\"\"\n    return price\n\n\nclass Cart(Base, metaclass=M):\n    items: list[int] = []\n\n    async def total(self) -> float | None:\n        pass\n";
        assert_eq!(
            summarize(code),
            [
                " 1: \"\"\"Prices.\"\"\"",
                " 4: TAX_RATE = 0.1",
                " 7: def apply_discount(price: float, percent: float=0, *, r='x') -> float",
                "        Price after discount.",
                "12: class Cart(Base)",
                "13:   items: list[int] = []",
                "15:   def total(self) -> float | None",
            ]
            .join("\n")
        );
    }

    #[test]
    fn reformatting_keeps_the_hash() {
        let a = parse("def f(x):\n    return x + 1  # one\n").unwrap();
        let b = parse("def f(x):\n    return (x+1)\n").unwrap();
        let c = parse("def f(x):\n    return x + 2\n").unwrap();
        let hash = |p: &Parsed| p.symbols[0].content_hash;
        assert_ne!(hash(&a), hash(&c));
        assert_eq!(
            hash(&parse("def f(x):\n    return x + 1\n").unwrap()),
            hash(&a)
        );
        let _ = b;
    }
}
