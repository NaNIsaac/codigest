//! Go outlines: the package clause with the package comment, functions and methods, types
//! with their fields or methods nested, and consts and vars.

use std::cell::RefCell;

use tree_sitter::{Node, Parser};

use super::entry::{Entry, one_line, render, signature, strip_block, text};
use super::python::shorten;

thread_local! {
    static PARSER: RefCell<Parser> = RefCell::new({
        let mut parser = Parser::new();
        parser.set_language(&tree_sitter_go::LANGUAGE.into()).expect("go grammar");
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
    walker.declarations(tree.root_node());
    render(&walker.out)
}

struct Walker<'a> {
    src: &'a str,
    out: Vec<Entry>,
}

impl Walker<'_> {
    /// The comment lines directly above `node`, without `//go:` style directives.
    fn doc(&self, node: Node) -> Option<String> {
        let mut lines = Vec::new();
        let mut below = node.start_position().row;
        let mut cur = node.prev_sibling();
        while let Some(comment) = cur.filter(|c| c.kind() == "comment") {
            if comment.end_position().row + 1 != below {
                break;
            }
            let raw = text(self.src, comment);
            lines.push(match raw.strip_prefix("//") {
                Some(line) if line.starts_with("go:") || line.starts_with("+build") => {
                    String::new()
                }
                Some(line) => line.to_string(),
                None => strip_block(raw),
            });
            below = comment.start_position().row;
            cur = comment.prev_sibling();
        }
        lines.reverse();
        let doc = lines.join("\n");
        (!doc.trim().is_empty()).then_some(doc)
    }

    fn push(&mut self, node: Node, depth: usize, head: String, doc: Option<String>) {
        self.out.push(Entry {
            line: node.start_position().row + 1,
            depth,
            head,
            doc,
        });
    }

    fn declarations(&mut self, root: Node) {
        let src = self.src;
        let mut cursor = root.walk();
        for node in root.named_children(&mut cursor) {
            match node.kind() {
                "package_clause" => {
                    self.push(node, 0, one_line(text(src, node)), self.doc(node));
                }
                "function_declaration" | "method_declaration" => {
                    let body = node.child_by_field_name("body");
                    let cut = body.map_or(node.end_byte(), |b| b.start_byte());
                    let head = signature(src, node.start_byte(), cut);
                    self.push(node, 0, head, self.doc(node));
                }
                "type_declaration" | "const_declaration" | "var_declaration" => self.group(node),
                _ => {}
            }
        }
    }

    /// The specs of a `type`, `const` or `var` declaration, one entry each.
    fn group(&mut self, decl: Node) {
        let src = self.src;
        let keyword = decl.kind().trim_end_matches("_declaration");
        let mut specs = Vec::new();
        let mut stack = vec![decl];
        while let Some(node) = stack.pop() {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                match child.kind() {
                    "type_spec" | "type_alias" | "const_spec" | "var_spec" => specs.push(child),
                    "var_spec_list" => stack.push(child),
                    _ => {}
                }
            }
        }
        specs.sort_by_key(|s| s.start_byte());
        let grouped = text(src, decl)
            .trim_start_matches(keyword)
            .trim_start()
            .starts_with('(');
        for spec in specs {
            let doc = self
                .doc(spec)
                .or_else(|| if grouped { None } else { self.doc(decl) });
            let start = spec.start_byte();
            let value_field = if keyword == "type" { "type" } else { "value" };
            let head = match (keyword, spec.child_by_field_name(value_field)) {
                ("type", Some(ty)) => {
                    let name = signature(src, start, ty.start_byte());
                    let shape = match ty.kind() {
                        "struct_type" => "struct".to_string(),
                        "interface_type" => "interface".to_string(),
                        _ => shorten(&one_line(text(src, ty))),
                    };
                    format!("type {name} {shape}")
                }
                (_, Some(value)) => {
                    let name = signature(src, start, value.start_byte());
                    let name = name.trim_end_matches('=').trim_end();
                    format!(
                        "{keyword} {name} = {}",
                        shorten(&one_line(text(src, value)))
                    )
                }
                _ => format!("{keyword} {}", signature(src, start, spec.end_byte())),
            };
            self.push(spec, 0, head, doc);
            if keyword == "type"
                && let Some(ty) = spec.child_by_field_name("type")
            {
                self.members(ty);
            }
        }
    }

    fn members(&mut self, ty: Node) {
        if !matches!(ty.kind(), "struct_type" | "interface_type") {
            return;
        }
        let src = self.src;
        let mut cursor = ty.walk();
        let list = ty
            .named_children(&mut cursor)
            .find(|c| c.kind() == "field_declaration_list")
            .unwrap_or(ty);
        let mut cursor = list.walk();
        for member in list.named_children(&mut cursor) {
            if member.kind() != "comment" {
                self.push(member, 1, one_line(text(src, member)), self.doc(member));
            }
        }
    }
}
