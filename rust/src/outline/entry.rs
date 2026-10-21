//! What the tree-sitter outlines (Rust, JavaScript/TypeScript, Go) share: the line format of
//! `python::summarize`, and the text helpers for signatures and doc comments.

use tree_sitter::Node;

use super::python::first_line;

pub struct Entry {
    pub line: usize,
    pub depth: usize,
    pub head: String,
    pub doc: Option<String>,
}

/// Numbered like `scan -l`, members indented two spaces per level, the first line of the doc
/// comment under its definition.
pub fn render(entries: &[Entry]) -> String {
    let width = entries
        .iter()
        .map(|e| e.line)
        .max()
        .map_or(1, |n| n.to_string().len());
    let mut lines = Vec::new();
    for e in entries {
        let indent = "  ".repeat(e.depth);
        lines.push(format!("{:>width$}: {indent}{}", e.line, e.head));
        if let Some(doc) = e.doc.as_deref().filter(|d| !d.trim().is_empty()) {
            lines.push(format!(
                "{}  {indent}    {}",
                " ".repeat(width),
                first_line(doc)
            ));
        }
    }
    lines.join("\n")
}

/// Whitespace runs become one space. A line break right inside a bracket goes away with the
/// trailing comma before it, so a signature wrapped one parameter per line reads `(a, b)`.
pub fn one_line(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if !c.is_whitespace() {
            out.push(c);
            continue;
        }
        let mut broke = c == '\n';
        while let Some(&next) = chars.peek().filter(|n| n.is_whitespace()) {
            broke |= next == '\n';
            chars.next();
        }
        let opens = out.ends_with(['(', '[', '{']);
        let closes = chars.peek().is_some_and(|n| matches!(n, ')' | ']' | '}'));
        if broke && closes && out.ends_with(',') {
            out.pop();
        }
        if !(out.is_empty() || broke && (opens || closes)) {
            out.push(' ');
        }
    }
    out.trim_end().to_string()
}

/// The text of a `/* */` comment without its markers and the stars down the left side.
pub fn strip_block(text: &str) -> String {
    let body = text.trim();
    let body = body.strip_prefix("/*").unwrap_or(body);
    let body = body.trim_start_matches(['*', '!']);
    let body = body.strip_suffix("*/").unwrap_or(body);
    body.lines()
        .map(|l| {
            l.trim_start()
                .strip_prefix('*')
                .unwrap_or_else(|| l.trim_start())
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn text<'a>(src: &'a str, node: Node) -> &'a str {
    &src[node.start_byte()..node.end_byte()]
}

/// `src[from..to]` on one line, without a trailing `;` or `,`.
pub fn signature(src: &str, from: usize, to: usize) -> String {
    one_line(&src[from..to])
        .trim_end_matches([';', ',', ' '])
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapped_signatures_join_without_stray_spaces() {
        assert_eq!(
            one_line("fn f(\n    a: i32,\n    b: i32,\n) -> i32\nwhere\n    T: Clone"),
            "fn f(a: i32, b: i32) -> i32 where T: Clone"
        );
        assert_eq!(one_line("a   b\t c"), "a b c");
        assert_eq!(one_line("f(a, )"), "f(a, )");
    }

    #[test]
    fn block_comments_lose_their_markers() {
        assert_eq!(
            strip_block("/**\n * One.\n * Two.\n */").trim(),
            "One.\n Two."
        );
        assert_eq!(strip_block("/*! Inner */").trim(), "Inner");
    }
}
