//! Output formats besides the XML blocks: Markdown for people and chats that render it,
//! JSON for scripts.

use clap::ValueEnum;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum Format {
    /// Tagged blocks, the same as the Python build
    #[default]
    Xml,
    /// Headings and fenced code blocks
    Markdown,
    /// One JSON document
    Json,
}

/// A code fence longer than any run of backticks inside `content`, so the block cannot be
/// closed early by the content itself.
pub fn fence(content: &str) -> String {
    let mut longest = 0;
    let mut run = 0;
    for c in content.chars() {
        if c == '`' {
            run += 1;
            longest = longest.max(run);
        } else {
            run = 0;
        }
    }
    "`".repeat((longest + 1).max(3))
}

/// A fenced block with a language tag taken from a file name.
pub fn fenced(content: &str, file_name: &str) -> String {
    let fence = fence(content);
    let body = content.strip_suffix('\n').unwrap_or(content);
    format!("{fence}{}\n{body}\n{fence}", language(file_name))
}

/// The info string most renderers know for a file's extension; empty when unknown.
pub fn language(file_name: &str) -> &'static str {
    let ext = file_name
        .rsplit_once('.')
        .map_or("", |(_, e)| e)
        .to_ascii_lowercase();
    match ext.as_str() {
        "py" | "pyi" => "python",
        "rs" => "rust",
        "js" | "mjs" | "cjs" => "javascript",
        "jsx" => "jsx",
        "ts" => "typescript",
        "tsx" => "tsx",
        "go" => "go",
        "m" => "matlab",
        "c" | "h" => "c",
        "cpp" | "cc" | "hpp" | "cxx" => "cpp",
        "java" => "java",
        "sh" | "bash" => "bash",
        "ps1" | "psm1" => "powershell",
        "json" => "json",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",
        "md" => "markdown",
        "html" => "html",
        "css" => "css",
        "sql" => "sql",
        "xml" => "xml",
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fence_outgrows_backticks_in_the_content() {
        assert_eq!(fence("plain"), "```");
        assert_eq!(fence("a ```` b"), "`````");
        assert_eq!(fenced("x = 1\n", "a.py"), "```python\nx = 1\n```");
    }
}
