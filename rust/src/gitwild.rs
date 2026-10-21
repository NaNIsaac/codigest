//! pathspec's `gitwildmatch` dialect, which `scanner.py` builds its ignore rules with,
//! reproduced rule by rule so both builds skip exactly the same files. The `ignore` crate
//! differs in the edge cases (`!` below an ignored folder, `**` forms), which would change
//! what a project shares.

use anyhow::{Context, Result, anyhow};
use regex::RegexSet;

const DIR_MARK: &str = "(/)";
const DIR_MARK_OPT: &str = "(?:(/)|$)";

enum PatternError {
    /// Git discards a pattern whose `[...]` is not closed.
    Range,
    Invalid(String),
}

pub struct GitWildSpec {
    set: RegexSet,
    include: Vec<bool>,
}

impl GitWildSpec {
    /// The last matching pattern decides; `!` patterns include instead of exclude.
    pub fn from_lines<S: AsRef<str>>(lines: impl IntoIterator<Item = S>) -> Result<Self> {
        let mut regexes = Vec::new();
        let mut include = Vec::new();
        for line in lines {
            let line = line.as_ref();
            match pattern_to_regex(line) {
                Ok(Some((regex, keep))) => {
                    regexes.push(regex);
                    include.push(keep);
                }
                Ok(None) | Err(PatternError::Range) => {}
                Err(PatternError::Invalid(why)) => {
                    return Err(anyhow!("Invalid git pattern: {line:?} ({why})"));
                }
            }
        }
        let set = RegexSet::new(&regexes).context("Invalid git pattern")?;
        Ok(Self { set, include })
    }

    /// `PathSpec.match_file`: `file` is relative to the root with `/` separators and a
    /// trailing `/` for a folder.
    pub fn match_file(&self, file: &str) -> bool {
        let file = file
            .strip_prefix('/')
            .or_else(|| file.strip_prefix("./"))
            .unwrap_or(file);
        self.set
            .matches(file)
            .iter()
            .next_back()
            .is_some_and(|i| self.include[i])
    }
}

fn pattern_to_regex(pattern: &str) -> Result<Option<(String, bool)>, PatternError> {
    // A space can be escaped, so only a pattern not ending in `\ ` loses trailing blanks.
    let mut text = if pattern.ends_with("\\ ") {
        pattern
    } else {
        crate::pystr::trim_end(pattern)
    };
    if text.is_empty() || text.starts_with('#') || text == "/" {
        return Ok(None);
    }
    let include = match text.strip_prefix('!') {
        Some(rest) => {
            text = rest;
            false
        }
        None => true,
    };

    let mut segments: Vec<String> = text.split('/').map(str::to_owned).collect();
    let is_dir = segments.last().is_some_and(String::is_empty);

    if let Some(regex) = normalize(is_dir, &mut segments).map_err(PatternError::Invalid)? {
        return Ok(Some((regex.to_owned(), include)));
    }
    let regex = translate(is_dir, &segments)?;
    Ok(Some((regex, include)))
}

/// Brings the segments to a form `translate` can walk; `Some` is a regex that stands for
/// the whole pattern (`*`, `**`, `*/`, `**/`).
fn normalize(is_dir: bool, segments: &mut Vec<String>) -> Result<Option<&'static str>, String> {
    if segments[0].is_empty() {
        // Anchored to the root.
        segments.remove(0);
    } else if (segments.len() == 1 || (segments.len() == 2 && segments[1].is_empty()))
        && segments[0] != "**"
    {
        segments.insert(0, "**".to_owned());
    }
    if segments.is_empty() {
        return Err("Pattern normalized to nothing.".to_owned());
    }
    if let Some(last) = segments.last_mut().filter(|s| s.is_empty()) {
        *last = "**".to_owned();
    }
    for i in (1..segments.len()).rev() {
        if segments[i - 1] == "**" && segments[i] == "**" {
            segments.remove(i);
        }
    }
    let seg: Vec<&str> = segments.iter().map(String::as_str).collect();
    Ok(match seg.as_slice() {
        ["**"] if is_dir => Some(DIR_MARK),
        ["**"] | ["**", "*"] => Some("."),
        ["**", "*", "**"] if is_dir => Some(DIR_MARK),
        ["**", "*", "**"] => Some("/"),
        _ => None,
    })
}

fn translate(is_dir: bool, segments: &[String]) -> Result<String, PatternError> {
    let mut out = String::new();
    let mut need_slash = false;
    let end = segments.len() - 1;
    for (i, seg) in segments.iter().enumerate() {
        if seg == "**" {
            if i == 0 {
                out.push_str("^(?:.+/)?");
            } else if i < end {
                out.push_str("(?:/.+)?");
                need_slash = true;
            } else if is_dir {
                out.push_str(DIR_MARK);
            } else {
                out.push('/');
            }
            continue;
        }
        if i == 0 {
            out.push('^');
        }
        if need_slash {
            out.push('/');
        }
        if seg == "*" {
            out.push_str("[^/]+");
        } else {
            out.push_str(&translate_glob(seg)?);
        }
        if i == end {
            out.push_str(DIR_MARK_OPT);
        }
        need_slash = true;
    }
    Ok(out)
}

/// One path segment: `*`, `?`, `[...]` and backslash escapes become regex.
fn translate_glob(pattern: &str) -> Result<String, PatternError> {
    let chars: Vec<char> = pattern.chars().collect();
    let end = chars.len();
    let literal = |c: char| regex::escape(c.encode_utf8(&mut [0; 4]));
    let mut out = String::new();
    let mut escaped = false;
    let mut i = 0;
    while i < end {
        let c = chars[i];
        i += 1;
        if escaped {
            escaped = false;
            out.push_str(&literal(c));
        } else if c == '\\' {
            escaped = true;
        } else if c == '*' {
            out.push_str("[^/]*");
        } else if c == '?' {
            out.push_str("[^/]");
        } else if c == '[' {
            let mut j = i;
            if j < end && (chars[j] == '!' || chars[j] == '^') {
                j += 1;
            }
            if j < end && chars[j] == ']' {
                j += 1;
            }
            while j < end && chars[j] != ']' {
                j += 1;
            }
            if j >= end {
                return Err(PatternError::Range);
            }
            j += 1;
            out.push('[');
            if chars[i] == '!' || chars[i] == '^' {
                out.push('^');
                i += 1;
            }
            // Python reads `[`, `&` and `~` inside a class literally; the regex crate
            // would start a nested class or a set operation.
            for &b in &chars[i..j - 1] {
                if matches!(b, ']' | '\\' | '[' | '&' | '~') {
                    out.push('\\');
                }
                out.push(b);
            }
            out.push(']');
            i = j;
        } else {
            out.push_str(&literal(c));
        }
    }
    if escaped {
        return Err(PatternError::Invalid(
            "Escape character found with no next character to escape".to_owned(),
        ));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(lines: &[&str]) -> GitWildSpec {
        GitWildSpec::from_lines(lines).unwrap()
    }

    #[test]
    fn a_bare_name_matches_at_any_depth_and_below() {
        let s = spec(&["build"]);
        assert!(s.match_file("build"));
        assert!(s.match_file("build/"));
        assert!(s.match_file("a/build/x.py"));
        assert!(!s.match_file("rebuild"));
        assert!(!s.match_file("build.py"));
    }

    #[test]
    fn a_trailing_slash_means_folders_only() {
        let s = spec(&["dist/"]);
        assert!(s.match_file("dist/"));
        assert!(s.match_file("dist/a.js"));
        assert!(s.match_file("pkg/dist/"));
        assert!(!s.match_file("dist"));
    }

    #[test]
    fn a_leading_slash_anchors_to_the_root() {
        let s = spec(&["/todo.txt", "src/gen"]);
        assert!(s.match_file("todo.txt"));
        assert!(!s.match_file("a/todo.txt"));
        assert!(s.match_file("src/gen/x.py"));
        assert!(!s.match_file("a/src/gen/x.py"));
    }

    #[test]
    fn wildcards_stay_within_a_segment() {
        let s = spec(&["*.lock", "a/*/c.py", "t?st.py", "[ab]x.py"]);
        assert!(s.match_file("pkg/yarn.lock"));
        assert!(s.match_file("a/b/c.py"));
        assert!(!s.match_file("a/b/d/c.py"));
        assert!(s.match_file("test.py"));
        assert!(s.match_file("bx.py"));
        assert!(!s.match_file("cx.py"));
    }

    #[test]
    fn double_asterisk_spans_folders() {
        let s = spec(&["a/**/z.py", "docs/**"]);
        assert!(s.match_file("a/z.py"));
        assert!(s.match_file("a/b/c/z.py"));
        assert!(s.match_file("docs/x/y.md"));
        assert!(!s.match_file("docs"));
    }

    #[test]
    fn the_last_matching_pattern_wins() {
        let s = spec(&["*.log", "!keep.log"]);
        assert!(s.match_file("a.log"));
        assert!(!s.match_file("keep.log"));
        let s = spec(&["!keep.log", "*.log"]);
        assert!(s.match_file("keep.log"));
    }

    #[test]
    fn negation_reincludes_below_an_ignored_folder_like_pathspec() {
        let s = spec(&["out/", "!out/keep.txt"]);
        assert!(!s.match_file("out/keep.txt"));
    }

    #[test]
    fn comments_blanks_and_lone_slash_do_nothing() {
        let s = spec(&["# note", "   ", "/", ""]);
        assert!(!s.match_file("note"));
        assert!(!s.match_file("a"));
    }

    #[test]
    fn a_star_alone_matches_everything() {
        assert!(spec(&["*"]).match_file("a/b"));
        assert!(spec(&["**"]).match_file("a"));
        assert!(spec(&["*/"]).match_file("a/b"));
        assert!(!spec(&["*/"]).match_file("a"));
    }

    #[test]
    fn escapes_and_unclosed_brackets() {
        let s = spec(&[r"\#file", r"a\ b", "[unclosed"]);
        assert!(s.match_file("#file"));
        assert!(s.match_file("a b"));
        assert!(!s.match_file("[unclosed"));
    }

    #[test]
    fn trailing_blanks_are_dropped_unless_escaped() {
        let s = spec(&["x.py  ", r"y.py\ "]);
        assert!(s.match_file("x.py"));
        assert!(s.match_file("y.py "));
        assert!(!s.match_file("y.py"));
    }

    #[test]
    fn invalid_patterns_are_errors() {
        assert!(GitWildSpec::from_lines(["!"]).is_err());
        assert!(GitWildSpec::from_lines([r"foo\"]).is_err());
    }
}
