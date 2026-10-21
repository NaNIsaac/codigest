//! Reading a file into the text that goes in its `<file>` block, as `core/processor.py`.

use std::fs;
use std::io;
use std::path::Path;

use anyhow::{Result, bail};

use crate::pypath;
use crate::pystr;

const BINARY: &str = "<<Binary or Non-UTF8 Content>>";

/// `Path.read_text(encoding="utf-8")`: strict UTF-8, newlines normalised to `\n`.
fn read_text(path: &Path) -> io::Result<Option<String>> {
    let bytes = fs::read(path)?;
    Ok(String::from_utf8(bytes)
        .ok()
        .map(|text| pystr::universal_newlines(&text)))
}

/// The file's text, optionally with right-aligned line numbers. Unreadable files come back
/// as a marker instead of an error, so one of them never stops a snapshot.
pub fn read_file_content(path: &Path, add_line_numbers: bool) -> String {
    let content = match read_text(path) {
        Ok(Some(text)) => text,
        Ok(None) => return BINARY.to_string(),
        Err(err) => return format!("<<Error: {err}>>"),
    };
    if !add_line_numbers {
        return content;
    }
    let lines = pystr::splitlines(&content);
    let width = lines.len().to_string().len();
    lines
        .iter()
        .enumerate()
        .map(|(i, line)| format!("{:>width$}: {line}", i + 1))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Lines `lo..=hi` (1-based), always numbered, and the range actually shown. A range past
/// the end is cut to the file; `hi = None` means the single line `lo`.
pub fn read_line_range(path: &Path, lo: usize, hi: Option<usize>) -> Result<(String, String)> {
    let Some(text) = read_text(path)? else {
        return Ok((BINARY.to_string(), String::new()));
    };
    let rows = pystr::splitlines(&text);
    let hi = hi.unwrap_or(lo).min(rows.len());
    if lo > hi {
        return Ok((
            format!("<<{} has {} lines>>", pypath::name(path), rows.len()),
            String::new(),
        ));
    }
    let width = hi.to_string().len();
    let mut body = Vec::with_capacity(hi - lo + 1);
    for i in lo..=hi {
        // `rows[i - 1]` with i = 0 is the last line in the original.
        let Some(row) = i
            .checked_sub(1)
            .map_or_else(|| rows.last(), |at| rows.get(at))
        else {
            bail!("list index out of range");
        };
        body.push(format!("{i:>width$}: {row}"));
    }
    let shown = if lo == hi {
        lo.to_string()
    } else {
        format!("{lo}-{hi}")
    };
    Ok((body.join("\n"), shown))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(content: &[u8]) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("f.txt");
        fs::write(&path, content).unwrap();
        (dir, path)
    }

    #[test]
    fn plain_content_has_native_newlines_folded() {
        let (_d, p) = file(b"a\r\nb\rc\n");
        assert_eq!(read_file_content(&p, false), "a\nb\nc\n");
    }

    #[test]
    fn line_numbers_pad_to_the_widest() {
        let rows: String = (1..=10).map(|i| format!("l{i}\n")).collect();
        let (_d, p) = file(rows.as_bytes());
        let numbered = read_file_content(&p, true);
        assert!(numbered.starts_with(" 1: l1\n 2: l2"));
        assert!(numbered.ends_with("10: l10"));
    }

    #[test]
    fn empty_file_numbers_to_nothing() {
        let (_d, p) = file(b"");
        assert_eq!(read_file_content(&p, true), "");
    }

    #[test]
    fn form_feed_breaks_a_numbered_line_like_splitlines() {
        let (_d, p) = file(b"a\x0cb\n");
        assert_eq!(read_file_content(&p, true), "1: a\n2: b");
    }

    #[test]
    fn invalid_utf8_is_a_marker() {
        let (_d, p) = file(&[0xff, 0xfe, 0x00]);
        assert_eq!(read_file_content(&p, false), BINARY);
        assert_eq!(read_line_range(&p, 1, None).unwrap().0, BINARY);
    }

    #[test]
    fn ranges_are_numbered_and_cut_to_the_file() {
        let (_d, p) = file(b"a\nb\nc\n");
        assert_eq!(
            read_line_range(&p, 2, Some(3)).unwrap(),
            ("2: b\n3: c".into(), "2-3".into())
        );
        assert_eq!(
            read_line_range(&p, 2, None).unwrap(),
            ("2: b".into(), "2".into())
        );
        assert_eq!(read_line_range(&p, 3, Some(99)).unwrap().1, "3");
        assert_eq!(
            read_line_range(&p, 9, None).unwrap(),
            ("<<f.txt has 3 lines>>".into(), String::new())
        );
    }

    #[test]
    fn range_numbers_align_on_the_last_one() {
        let rows: String = (1..=12).map(|i| format!("l{i}\n")).collect();
        let (_d, p) = file(rows.as_bytes());
        let (body, shown) = read_line_range(&p, 9, Some(11)).unwrap();
        assert_eq!(body, " 9: l9\n10: l10\n11: l11");
        assert_eq!(shown, "9-11");
    }

    #[test]
    fn line_zero_wraps_to_the_last_line() {
        let (_d, p) = file(b"a\nb\n");
        assert_eq!(read_line_range(&p, 0, Some(1)).unwrap().0, "0: b\n1: a");
    }
}
