//! Python `str` behaviour the output depends on: which characters count as whitespace,
//! where `splitlines` breaks, and the number formats of `f"{n:,}"` and `rich.filesize`.

/// `str.isspace`: Rust's definition plus the four information separators.
pub fn is_space(c: char) -> bool {
    c.is_whitespace() || ('\x1c'..='\x1f').contains(&c)
}

pub fn trim(text: &str) -> &str {
    text.trim_matches(is_space)
}

pub fn trim_end(text: &str) -> &str {
    text.trim_end_matches(is_space)
}

/// What text-mode reading does: `\r\n` and a lone `\r` both become `\n`.
pub fn universal_newlines(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

/// `str.splitlines()`: breaks at more than `\n` (form feed, the separators, NEL, U+2028/9)
/// and drops the terminator; a final terminator does not start another line.
pub fn splitlines(text: &str) -> Vec<&str> {
    let mut lines = Vec::new();
    let mut start = 0;
    let mut chars = text.char_indices().peekable();
    while let Some((at, c)) = chars.next() {
        let next = match c {
            '\r' if matches!(chars.peek(), Some(&(_, '\n'))) => {
                chars.next();
                at + 2
            }
            '\r'
            | '\n'
            | '\x0b'
            | '\x0c'
            | '\x1c'..='\x1e'
            | '\u{85}'
            | '\u{2028}'
            | '\u{2029}' => at + c.len_utf8(),
            _ => continue,
        };
        lines.push(&text[start..at]);
        start = next;
    }
    if start < text.len() {
        lines.push(&text[start..]);
    }
    lines
}

/// `f"{n:,}"`.
pub fn thousands(n: u64) -> String {
    group(&n.to_string())
}

fn group(digits: &str) -> String {
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// `rich.filesize.decimal`.
pub fn filesize(size: u64) -> String {
    if size == 1 {
        return "1 byte".to_string();
    }
    if size < 1000 {
        return format!("{size} bytes");
    }
    const SUFFIXES: [&str; 8] = ["kB", "MB", "GB", "TB", "PB", "EB", "ZB", "YB"];
    let mut unit = 1000f64;
    let mut last = SUFFIXES[0];
    for suffix in SUFFIXES {
        unit *= 1000.0;
        last = suffix;
        // The first suffix is picked by size < 1000^2, as `enumerate(suffixes, 2)` does.
        if (size as f64) < unit {
            break;
        }
    }
    let value = format!("{:.1}", 1000.0 * size as f64 / unit);
    let (whole, fraction) = value.split_once('.').unwrap_or((&value, "0"));
    format!("{}.{fraction} {last}", group(whole))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitlines_breaks_like_python() {
        assert_eq!(
            splitlines("a\nb\r\nc\rd\x0ce\u{2028}f"),
            ["a", "b", "c", "d", "e", "f"]
        );
        assert_eq!(splitlines("a\n"), ["a"]);
        assert_eq!(splitlines("a\n\n"), ["a", ""]);
        assert!(splitlines("").is_empty());
    }

    #[test]
    fn whitespace_includes_information_separators() {
        assert_eq!(trim("\x1c a \u{a0}"), "a");
    }

    #[test]
    fn sizes_read_like_rich() {
        assert_eq!(filesize(1), "1 byte");
        assert_eq!(filesize(999), "999 bytes");
        assert_eq!(filesize(1000), "1.0 kB");
        assert_eq!(filesize(123_456), "123.5 kB");
        assert_eq!(filesize(2_500_000), "2.5 MB");
        assert_eq!(thousands(1234567), "1,234,567");
    }
}
