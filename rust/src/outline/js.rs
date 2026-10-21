//! JavaScript, TypeScript and TSX outlines: functions, classes with their members,
//! interfaces, type aliases, enums, namespaces and exported values, with the first line of
//! the JSDoc block just above each.

use std::cell::RefCell;

use tree_sitter::{Language, Node, Parser};

use super::entry::{Entry, one_line, render, signature, strip_block, text};
use super::python::{first_line, shorten};

thread_local! {
    static PARSER: RefCell<Parser> = RefCell::new(Parser::new());
}

fn language(extension: &str) -> Language {
    match extension {
        "ts" => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        "tsx" => tree_sitter_typescript::LANGUAGE_TSX.into(),
        _ => tree_sitter_javascript::LANGUAGE.into(),
    }
}

pub fn summarize(extension: &str, code: &str) -> String {
    let tree = PARSER.with_borrow_mut(|parser| {
        parser.set_language(&language(extension)).ok()?;
        parser.parse(code, None)
    });
    let Some(tree) = tree else {
        return String::new();
    };
    let mut walker = Walker {
        src: code,
        out: Vec::new(),
    };
    let root = tree.root_node();
    walker.file_doc(root);
    walker.statements(root, 0);
    render(&walker.out)
}

struct Walker<'a> {
    src: &'a str,
    out: Vec<Entry>,
}

const FUNCTION_VALUES: [&str; 5] = [
    "arrow_function",
    "function_expression",
    "function",
    "generator_function",
    "class",
];

/// Where the declaration text starts: after any decorators.
fn start_of(node: Node) -> usize {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .find(|c| c.kind() != "decorator")
        .map_or(node.start_byte(), |c| c.start_byte())
}

fn row_of(src: &str, byte: usize) -> usize {
    src[..byte].matches('\n').count() + 1
}

fn is_jsdoc(comment: Node, src: &str) -> bool {
    let raw = text(src, comment);
    comment.kind() == "comment"
        && raw.starts_with("/**")
        && !raw.starts_with("/**/")
        && !raw.starts_with("/***")
}

/// `name = value` pieces: the text before the value without its `=`.
fn before_value(src: &str, from: usize, value: Node) -> String {
    let before = signature(src, from, value.start_byte());
    before.trim_end_matches('=').trim_end().to_string()
}

/// A function or class value cut at its body, anything else shortened.
fn value_text(src: &str, value: Node) -> String {
    if FUNCTION_VALUES.contains(&value.kind()) {
        let body = value.child_by_field_name("body");
        signature(
            src,
            value.start_byte(),
            body.map_or(value.end_byte(), |b| b.start_byte()),
        )
    } else {
        shorten(&one_line(text(src, value)))
    }
}

impl Walker<'_> {
    /// A `/** */` block at the top of the file that a blank line separates from the code.
    fn file_doc(&mut self, root: Node) {
        let mut cursor = root.walk();
        let mut children = root
            .children(&mut cursor)
            .filter(|c| c.kind() != "hash_bang_line");
        let Some(first) = children.next().filter(|c| is_jsdoc(*c, self.src)) else {
            return;
        };
        if children
            .next()
            .is_some_and(|n| n.start_position().row <= first.end_position().row + 1)
        {
            return;
        }
        let doc = first_line(&strip_block(text(self.src, first)));
        if doc.is_empty() {
            return;
        }
        self.out.push(Entry {
            line: first.start_position().row + 1,
            depth: 0,
            head: format!("/** {doc} */"),
            doc: None,
        });
    }

    fn doc(&self, node: Node) -> Option<String> {
        let comment = node.prev_sibling()?;
        (is_jsdoc(comment, self.src) && comment.end_position().row + 1 == node.start_position().row)
            .then(|| strip_block(text(self.src, comment)))
    }

    fn push(&mut self, line: usize, depth: usize, head: String, doc: Option<String>) {
        self.out.push(Entry {
            line,
            depth,
            head,
            doc,
        });
    }

    fn statements(&mut self, parent: Node, depth: usize) {
        let mut cursor = parent.walk();
        for node in parent.named_children(&mut cursor) {
            match node.kind() {
                "ERROR" => self.statements(node, depth),
                "expression_statement" => self.expression(node, depth),
                "export_statement" | "ambient_declaration" => self.wrapper(node, depth),
                _ => self.declaration(node, node, depth),
            }
        }
    }

    /// `namespace X {}`, which TypeScript parses as an expression statement, and the body of
    /// an immediately invoked function, where a plain script keeps its definitions.
    fn expression(&mut self, node: Node, depth: usize) {
        let mut cursor = node.walk();
        let Some(expr) = node
            .named_children(&mut cursor)
            .find(|c| c.kind() != "comment")
        else {
            return;
        };
        if matches!(expr.kind(), "internal_module" | "module") {
            self.declaration(node, expr, depth);
            return;
        }
        let mut callee = (expr.kind() == "call_expression")
            .then(|| expr.child_by_field_name("function"))
            .flatten();
        while let Some(inner) = callee.filter(|c| c.kind() == "parenthesized_expression") {
            let mut cursor = inner.walk();
            callee = inner.named_children(&mut cursor).next();
        }
        if let Some(body) = callee
            .filter(|c| {
                matches!(
                    c.kind(),
                    "function_expression" | "function" | "arrow_function"
                )
            })
            .and_then(|c| c.child_by_field_name("body"))
            .filter(|b| b.kind() == "statement_block")
        {
            self.statements(body, depth);
        }
    }

    /// `export ...` and `declare ...`: the declaration inside, headed with the keywords.
    fn wrapper(&mut self, outer: Node, depth: usize) {
        if let Some(decl) = outer.child_by_field_name("declaration") {
            self.declaration(outer, decl, depth);
            return;
        }
        let mut cursor = outer.walk();
        let inner = outer
            .named_children(&mut cursor)
            .find(|c| !matches!(c.kind(), "comment" | "decorator"));
        let Some(inner) = inner else { return };
        if outer.kind() == "ambient_declaration" {
            self.declaration(outer, inner, depth);
        } else if let Some(value) = outer.child_by_field_name("value")
            && FUNCTION_VALUES.contains(&value.kind())
        {
            // `export default function () {}`
            let start = start_of(outer);
            let head = signature(
                self.src,
                start,
                value
                    .child_by_field_name("body")
                    .map_or(value.end_byte(), |b| b.start_byte()),
            );
            let doc = self.doc(outer);
            self.push(row_of(self.src, start), depth, head, doc);
            if let Some(body) = value.child_by_field_name("body")
                && value.kind() == "class"
            {
                self.members(body, depth + 1);
            }
        }
    }

    fn declaration(&mut self, outer: Node, node: Node, depth: usize) {
        let src = self.src;
        let start = start_of(outer);
        let body = node.child_by_field_name("body");
        let until_body = |body: Option<Node>| {
            signature(src, start, body.map_or(node.end_byte(), |b| b.start_byte()))
        };
        let head = match node.kind() {
            "function_declaration"
            | "generator_function_declaration"
            | "function_signature"
            | "enum_declaration"
            | "class_declaration"
            | "abstract_class_declaration"
            | "interface_declaration"
            | "internal_module"
            | "module" => until_body(body),
            "type_alias_declaration" => match node.child_by_field_name("value") {
                Some(value) => format!(
                    "{} {}",
                    signature(src, start, value.start_byte()),
                    shorten(&one_line(text(src, value)))
                ),
                None => until_body(None),
            },
            "lexical_declaration" | "variable_declaration" => {
                self.variables(outer, node, depth);
                return;
            }
            _ => return,
        };
        let doc = self.doc(outer);
        self.push(row_of(src, start), depth, head, doc);
        if let Some(body) = body {
            match node.kind() {
                "class_declaration" | "abstract_class_declaration" | "interface_declaration" => {
                    self.members(body, depth + 1);
                }
                "internal_module" | "module" => self.statements(body, depth + 1),
                _ => {}
            }
        }
    }

    /// `const`/`let`/`var`: every declarator holding a function or class, and the rest when
    /// exported or declared.
    fn variables(&mut self, outer: Node, node: Node, depth: usize) {
        let src = self.src;
        let start = start_of(outer);
        let listed = outer.id() != node.id();
        let mut cursor = node.walk();
        let declarators: Vec<Node> = node
            .named_children(&mut cursor)
            .filter(|c| c.kind() == "variable_declarator")
            .collect();
        let Some(first) = declarators.first() else {
            return;
        };
        let lead = one_line(&src[start..first.start_byte()]);
        for decl in &declarators {
            let value = decl.child_by_field_name("value");
            let is_function = value.is_some_and(|v| FUNCTION_VALUES.contains(&v.kind()));
            if !listed && !is_function {
                continue;
            }
            let head = match value {
                None => signature(src, decl.start_byte(), decl.end_byte()),
                Some(v) => format!(
                    "{} = {}",
                    before_value(src, decl.start_byte(), v),
                    value_text(src, v)
                ),
            };
            let doc = self.doc(outer);
            self.push(
                row_of(src, decl.start_byte()),
                depth,
                format!("{lead} {head}"),
                doc,
            );
        }
    }

    fn members(&mut self, body: Node, depth: usize) {
        let src = self.src;
        let mut cursor = body.walk();
        for member in body.named_children(&mut cursor) {
            let start = start_of(member);
            let head = match member.kind() {
                "method_definition" => {
                    let cut = member.child_by_field_name("body");
                    signature(
                        src,
                        start,
                        cut.map_or(member.end_byte(), |b| b.start_byte()),
                    )
                }
                "field_definition" | "public_field_definition" => {
                    match member.child_by_field_name("value") {
                        Some(v) => {
                            format!("{} = {}", before_value(src, start, v), value_text(src, v))
                        }
                        None => signature(src, start, member.end_byte()),
                    }
                }
                "abstract_method_signature"
                | "method_signature"
                | "property_signature"
                | "index_signature"
                | "call_signature"
                | "construct_signature" => signature(src, start, member.end_byte()),
                _ => continue,
            };
            self.push(row_of(src, start), depth, head, self.doc(member));
        }
    }
}
