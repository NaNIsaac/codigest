//! MATLAB outlines (`core/outline.py`): functions and classdefs with their help line, and
//! `%%` section titles.

use std::sync::LazyLock;

use regex::Regex;

// `function out = name(a, b)`, `function [a, b] = name(x)`, `function name`
static DEF: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*(?P<kind>function|classdef)\b\s*(?:\[?[\w\s,~]*\]?\s*=\s*)?(?P<rest>[\w.]+\s*(?:\([^)]*\))?)")
        .expect("valid pattern")
});
static COMMENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*%\s?(?P<text>.*\S)").expect("valid pattern"));
// MATLAB needs the space: `%%%%` banners and `%%comment` are not sections.
static SECTION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*%% +\S").expect("valid pattern"));
static WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\w").expect("valid pattern"));

fn comment_text(line: &str) -> &str {
    line.trim().trim_start_matches('%').trim()
}

/// True when the `%%` line just above already had words of its own.
fn continues_title(lines: &[&str], i: usize) -> bool {
    i > 0 && SECTION.is_match(lines[i - 1]) && WORD.is_match(comment_text(lines[i - 1]))
}

/// The words of a `%%` line, or, for a `%% =====` banner, of the comment under it.
fn section_title(lines: &[&str], i: usize) -> String {
    let text = comment_text(lines[i]);
    if WORD.is_match(text) {
        return text.to_string();
    }
    if continues_title(lines, i) {
        return String::new(); // the closing banner under a title
    }
    for following in lines.iter().skip(i + 1).take(2) {
        if !following.trim_start().starts_with('%') {
            break;
        }
        let text = comment_text(following);
        if WORD.is_match(text) {
            return text.to_string();
        }
    }
    String::new()
}

pub fn summarize(code: &str) -> String {
    let lines = split_lines(code);
    let width = lines.len().to_string().len();
    let mut out: Vec<String> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if SECTION.is_match(line) {
            let title = section_title(&lines, i);
            // A title wrapped over several `%%` lines is one section.
            let repeated = out
                .last()
                .is_some_and(|last| last.ends_with(title.as_str()));
            if !title.is_empty() && !(repeated || continues_title(&lines, i)) {
                out.push(format!("{:>width$}: %% {title}", i + 1));
            }
            continue;
        }
        if !DEF.is_match(line) {
            continue;
        }
        out.push(format!("{:>width$}: {}", i + 1, line.trim()));
        if let Some(c) = lines.get(i + 1).and_then(|next| COMMENT.captures(next)) {
            out.push(format!("{}      {}", " ".repeat(width), &c["text"]));
        }
    }
    out.join("\n")
}

/// `str.splitlines()`: besides `\n` and `\r\n`, a lone `\r` and a few other characters end a line.
pub fn split_lines(text: &str) -> Vec<&str> {
    let mut lines = Vec::new();
    let mut start = 0;
    let mut chars = text.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        let end = match c {
            '\r' if chars.peek().is_some_and(|(_, n)| *n == '\n') => {
                chars.next();
                Some(i + 2)
            }
            '\n' | '\r' | '\x0b' | '\x0c' | '\x1c' | '\x1d' | '\x1e' | '\u{85}' | '\u{2028}'
            | '\u{2029}' => Some(i + c.len_utf8()),
            _ => None,
        };
        if let Some(end) = end {
            lines.push(&text[start..i]);
            start = end;
        }
    }
    if start < text.len() {
        lines.push(&text[start..]);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outline_lists_functions_help_lines_and_sections() {
        let code = "function [q, r] = det2lab(p, params)\n% detector frame to lab frame\n%% 1. Geometry\n%% continues the title\n%%%%%%%%%%%%\n%%not a section\nx = 1;\nfunction y = helper(x)\n";
        assert_eq!(
            summarize(code).lines().collect::<Vec<_>>(),
            [
                "1: function [q, r] = det2lab(p, params)",
                "       detector frame to lab frame",
                "3: %% 1. Geometry",
                "8: function y = helper(x)"
            ]
        );
    }

    #[test]
    fn banner_sections_take_the_title_under_them() {
        let code = "%% ==========\n%% Nominal voxel sizes\n%% ==========\n% computed from the detector distance\nx = 1;\n%% ----------\n% Interpolation\ny = 2;\n";
        assert_eq!(
            summarize(code).lines().collect::<Vec<_>>(),
            ["1: %% Nominal voxel sizes", "6: %% Interpolation"]
        );
    }

    #[test]
    fn splitlines_matches_python() {
        assert_eq!(split_lines("a\r\nb\rc\u{2028}d\n"), ["a", "b", "c", "d"]);
        assert_eq!(split_lines("a\n\nb"), ["a", "", "b"]);
    }
}
