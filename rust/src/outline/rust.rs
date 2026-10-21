//! Rust outlines: items with their signature as written, `impl` and `trait` members nested,
//! and the first line of each item's `///` or `#[doc]` text.

use std::cell::RefCell;

use tree_sitter::{Node, Parser};

use super::entry::{Entry, one_line, render, signature, strip_block, text};
use super::python::shorten;

thread_local! {
    static PARSER: RefCell<Parser> = RefCell::new({
        let mut parser = Parser::new();
        parser.set_language(&tree_sitter_rust::LANGUAGE.into()).expect("rust grammar");
        parser
    });
}

pub fn summarize(code: &str) -> String {
    let Some(tree) = PARSER.with_borrow_mut(|parser| parser.parse(code, None)) else {
        return String::new();
    };
    let mut walker = Walker {
        src: code,
        out: Vec::new(),
    };
    walker.module_doc(tree.root_node());
    walker.items(tree.root_node(), 0);
    render(&walker.out)
}

struct Walker<'a> {
    src: &'a str,
    out: Vec<Entry>,
}

/// `#[doc = "text"]` and `#![doc = "text"]`; `#[doc(hidden)]` and the like have no text.
fn doc_attribute(attr: &str) -> Option<String> {
    let rest = attr
        .trim()
        .strip_prefix("#!")
        .or_else(|| attr.trim().strip_prefix('#'))?;
    let rest = rest.strip_prefix("[doc")?.trim_start().strip_prefix('=')?;
    let value = rest.trim().strip_suffix(']')?.trim();
    let value = value.strip_prefix('"')?.strip_suffix('"')?;
    Some(value.replace("\\\"", "\"").replace("\\\\", "\\"))
}

/// The text of a doc comment of the given kind (`/` outer, `!` inner), or None for any other.
fn doc_comment(node: Node, src: &str, marker: char) -> Option<String> {
    let raw = text(src, node).trim_end();
    match node.kind() {
        "line_comment" => {
            let rest = raw.strip_prefix("//")?.strip_prefix(marker)?;
            (marker != '/' || !rest.starts_with('/')).then(|| rest.to_string())
        }
        "block_comment" => {
            let opens = if marker == '/' { "/**" } else { "/*!" };
            let plain = marker == '/' && (raw.starts_with("/***") || raw == "/**/");
            (raw.starts_with(opens) && !plain).then(|| strip_block(raw))
        }
        _ => None,
    }
}

impl Walker<'_> {
    /// `//!` lines (and `#![doc]`) before the first item.
    fn module_doc(&mut self, root: Node) {
        let mut lines = Vec::new();
        let mut first = None;
        let mut cursor = root.walk();
        for child in root.children(&mut cursor) {
            let doc = match child.kind() {
                "line_comment" | "block_comment" => doc_comment(child, self.src, '!'),
                "inner_attribute_item" => doc_attribute(text(self.src, child)),
                _ => break,
            };
            if let Some(doc) = doc {
                first.get_or_insert(child.start_position().row + 1);
                lines.push(doc);
            }
        }
        if let Some(line) = first {
            let doc = crate::outline::python::first_line(&lines.join("\n"));
            self.out.push(Entry {
                line,
                depth: 0,
                head: format!("//! {doc}"),
                doc: None,
            });
        }
    }

    /// The doc lines above an item, skipping its other attributes.
    fn doc(&self, item: Node) -> Option<String> {
        let mut lines = Vec::new();
        let mut cur = item.prev_sibling();
        while let Some(node) = cur {
            match node.kind() {
                "line_comment" | "block_comment" => match doc_comment(node, self.src, '/') {
                    Some(doc) => lines.push(doc),
                    None => break,
                },
                "attribute_item" => lines.extend(doc_attribute(text(self.src, node))),
                _ => break,
            }
            cur = node.prev_sibling();
        }
        lines.reverse();
        (!lines.is_empty()).then(|| lines.join("\n"))
    }

    fn is_test_only(&self, item: Node) -> bool {
        let mut cur = item.prev_sibling();
        while let Some(node) = cur {
            match node.kind() {
                "attribute_item" => {
                    if one_line(text(self.src, node)).replace(' ', "") == "#[cfg(test)]" {
                        return true;
                    }
                }
                "line_comment" | "block_comment" => {}
                _ => break,
            }
            cur = node.prev_sibling();
        }
        false
    }

    fn items(&mut self, parent: Node, depth: usize) {
        let mut cursor = parent.walk();
        for node in parent.named_children(&mut cursor) {
            if node.kind() == "ERROR" {
                self.items(node, depth);
            } else {
                self.item(node, depth);
            }
        }
    }

    fn item(&mut self, node: Node, depth: usize) {
        let src = self.src;
        let body = node.child_by_field_name("body");
        let braced = body.filter(|b| {
            matches!(
                b.kind(),
                "block" | "declaration_list" | "field_declaration_list" | "enum_variant_list"
            )
        });
        let head = match node.kind() {
            "function_item" | "impl_item" | "trait_item" | "mod_item" | "foreign_mod_item"
            | "struct_item" | "enum_item" | "union_item" => signature(
                src,
                node.start_byte(),
                braced.map_or(node.end_byte(), |b| b.start_byte()),
            ),
            "function_signature_item" | "associated_type" => {
                signature(src, node.start_byte(), node.end_byte())
            }
            "const_item" | "static_item" | "type_item" => {
                let field = if node.kind() == "type_item" {
                    "type"
                } else {
                    "value"
                };
                match node.child_by_field_name(field) {
                    Some(value) => {
                        let before = signature(src, node.start_byte(), value.start_byte());
                        format!("{before} {}", shorten(&one_line(text(src, value))))
                    }
                    None => signature(src, node.start_byte(), node.end_byte()),
                }
            }
            "macro_definition" => {
                let name = node
                    .child_by_field_name("name")
                    .map_or("", |n| text(src, n));
                format!("macro_rules! {name}")
            }
            _ => return,
        };
        if self.is_test_only(node) {
            return;
        }
        self.out.push(Entry {
            line: node.start_position().row + 1,
            depth,
            head,
            doc: self.doc(node),
        });
        if matches!(
            node.kind(),
            "impl_item" | "trait_item" | "mod_item" | "foreign_mod_item"
        ) && let Some(body) = braced
        {
            self.items(body, depth + 1);
        }
    }
}
