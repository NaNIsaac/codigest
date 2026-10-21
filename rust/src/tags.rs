//! The XML wrapping shared by every command, matching `core/tags.py` and `html.escape`.

use std::sync::LazyLock;

use regex::Regex;

/// The elements Codigest wraps context in. Only a closing tag of one of these inside the
/// content could end a block early; everything else is left exactly as written.
pub const WRAPPERS: [&str; 14] = [
    "file",
    "source_code",
    "project_snapshot",
    "project_structure",
    "code_digest",
    "changes",
    "semantic_diff",
    "user_instruction",
    "context_pack",
    "omitted",
    "terminal_history",
    "command",
    "output",
    "note",
];

static CLOSING: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!("</({})>", WRAPPERS.join("|"))).expect("valid pattern"));

/// Content goes in verbatim: `a -> b` must reach the reader as `a -> b`, or it comes back
/// escaped in the edits a model suggests.
pub fn escape_value(value: &str) -> String {
    CLOSING.replace_all(value, "&lt;/${1}>").into_owned()
}

/// Python's `html.escape(s, quote=True)`, which every attribute value goes through.
pub fn escape_attr(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#x27;"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_closing_wrapper_tags_change() {
        assert_eq!(
            escape_value("a -> b </div> </file>"),
            "a -> b </div> &lt;/file>"
        );
    }

    #[test]
    fn attributes_escape_like_html_escape() {
        assert_eq!(
            escape_attr(r#"echo "it's" <a & b>"#),
            "echo &quot;it&#x27;s&quot; &lt;a &amp; b&gt;"
        );
    }
}
