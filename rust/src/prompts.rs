//! The wrapping around a command's context, and the `.codigest/prompts.toml` override, as
//! `core/prompts.py`. An override is a Python `str.format` template, so `{name}`, `{{`,
//! conversions and format specs behave as they do there.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use anyhow::{Result, anyhow};

use crate::tags::{escape_attr, escape_value};

/// Keyword arguments of a render call.
pub type Vars<'a> = &'a [(&'a str, &'a str)];

fn var<'a>(vars: Vars<'a>, name: &str) -> &'a str {
    vars.iter()
        .find(|(key, _)| *key == name)
        .map_or("", |(_, value)| *value)
}

fn instruction_block(instruction: &str) -> String {
    if instruction.is_empty() {
        return String::new();
    }
    format!(
        "<user_instruction>\n{}\n</user_instruction>",
        escape_value(instruction)
    )
}

/// One root element holding the context. No canned instructions to the model: what to do
/// with it is the user's `-m` (or their own prompts.toml template).
pub fn block(root: &str, attrs: &[(&str, &str)], instruction: &str, parts: &[String]) -> String {
    let head = attrs
        .iter()
        .filter(|(_, value)| !value.is_empty())
        .map(|(key, value)| format!("{key}=\"{}\"", escape_attr(value)))
        .collect::<Vec<_>>()
        .join(" ");
    let body: Vec<String> = std::iter::once(instruction_block(instruction))
        .chain(parts.iter().cloned())
        .filter(|part| !part.is_empty())
        .collect();
    format!("<{root} {head}>\n{}\n</{root}>", body.join("\n"))
}

fn default_snapshot(vars: Vars) -> String {
    block(
        "project_snapshot",
        &[("project", var(vars, "project_name"))],
        var(vars, "instruction"),
        &[
            format!(
                "<project_structure>\n{}\n</project_structure>",
                var(vars, "tree_structure")
            ),
            format!(
                "<source_code>\n{}\n</source_code>",
                var(vars, "source_code")
            ),
        ],
    )
}

fn default_diff(vars: Vars) -> String {
    block(
        "changes",
        &[
            ("project", var(vars, "project_name")),
            ("source", var(vars, "context_message")),
        ],
        var(vars, "instruction"),
        &[escape_value(var(vars, "diff_content"))],
    )
}

fn default_semdiff(vars: Vars) -> String {
    block(
        "semantic_diff",
        &[
            ("project", var(vars, "project_name")),
            ("source", var(vars, "context_message")),
        ],
        var(vars, "instruction"),
        &[var(vars, "semdiff_content").to_owned()],
    )
}

fn default_digest(vars: Vars) -> String {
    block(
        "code_digest",
        &[
            ("project", var(vars, "project_name")),
            ("contents", "signatures and docstrings, no bodies"),
        ],
        var(vars, "instruction"),
        &[
            format!(
                "<project_structure>\n{}\n</project_structure>",
                var(vars, "tree_structure")
            ),
            var(vars, "digest_content").to_owned(),
        ],
    )
}

pub struct PromptEngine {
    overrides: toml::Table,
}

pub fn get_engine(root: &Path) -> PromptEngine {
    PromptEngine {
        overrides: load_overrides(root),
    }
}

/// The `[prompts]` table of `.codigest/prompts.toml`; a missing or broken file overrides
/// nothing.
fn load_overrides(root: &Path) -> toml::Table {
    fs::read_to_string(root.join(".codigest").join("prompts.toml"))
        .ok()
        .and_then(|text| text.parse::<toml::Table>().ok())
        .and_then(|mut data| match data.remove("prompts") {
            Some(toml::Value::Table(table)) => Some(table),
            _ => None,
        })
        .unwrap_or_default()
}

impl PromptEngine {
    /// A template from prompts.toml wins over the built-in layout. Code blocks arrive
    /// already wrapped by `tags::file`; escaping them again would turn their own `<file>`
    /// tags into text, so only the instruction is escaped.
    pub fn render(&self, key: &str, vars: Vars) -> Result<String> {
        if let Some(raw) = self.overrides.get(key) {
            let toml::Value::String(template) = raw else {
                return Err(anyhow!(
                    "'{}' object has no attribute 'format'",
                    toml_type(raw)
                ));
            };
            let instruction = escape_value(var(vars, "instruction"));
            let mut safe: Vec<(&str, &str)> = vars
                .iter()
                .filter(|(name, _)| *name != "instruction")
                .copied()
                .collect();
            safe.push(("instruction", &instruction));
            return match format(template, &safe) {
                Ok(text) => Ok(text),
                Err(FormatError::Key(name)) => Ok(format!(
                    "Error rendering template '{key}': Missing argument {}",
                    py_repr(&name)
                )),
                Err(FormatError::Value(why)) => Err(anyhow!(why)),
            };
        }
        Ok(match key {
            "snapshot" => default_snapshot(vars),
            "diff" => default_diff(vars),
            "semdiff" => default_semdiff(vars),
            "digest" => default_digest(vars),
            _ => format!("Error: Prompt template '{key}' not found."),
        })
    }
}

fn toml_type(value: &toml::Value) -> &'static str {
    match value {
        toml::Value::String(_) => "str",
        toml::Value::Integer(_) => "int",
        toml::Value::Float(_) => "float",
        toml::Value::Boolean(_) => "bool",
        toml::Value::Datetime(_) => "datetime",
        toml::Value::Array(_) => "list",
        toml::Value::Table(_) => "dict",
    }
}

#[derive(Debug, PartialEq)]
pub enum FormatError {
    /// `KeyError`: the template names an argument that was not passed.
    Key(String),
    /// `ValueError`, `IndexError` and the like: the template itself is wrong.
    Value(String),
}

fn invalid<T>(why: impl Into<String>) -> Result<T, FormatError> {
    Err(FormatError::Value(why.into()))
}

/// `template.format(**vars)` for string arguments.
pub fn format(template: &str, vars: Vars) -> Result<String, FormatError> {
    let chars: Vec<char> = template.chars().collect();
    expand(&chars, vars, 2)
}

fn expand(template: &[char], vars: Vars, depth: u8) -> Result<String, FormatError> {
    if depth == 0 {
        return invalid("Max string recursion exceeded");
    }
    let mut out = String::new();
    let mut i = 0;
    while i < template.len() {
        let c = template[i];
        i += 1;
        match c {
            '{' if template.get(i) == Some(&'{') => {
                out.push('{');
                i += 1;
            }
            '{' => {
                if i >= template.len() {
                    return invalid("Single '{' encountered in format string");
                }
                let start = i;
                let mut nesting = 1;
                while i < template.len() {
                    match template[i] {
                        '{' => nesting += 1,
                        '}' => {
                            nesting -= 1;
                            if nesting == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                    i += 1;
                }
                if nesting != 0 {
                    return invalid("expected '}' before end of string");
                }
                out.push_str(&replace_field(&template[start..i], vars, depth)?);
                i += 1;
            }
            '}' if template.get(i) == Some(&'}') => {
                out.push('}');
                i += 1;
            }
            '}' => return invalid("Single '}' encountered in format string"),
            _ => out.push(c),
        }
    }
    Ok(out)
}

fn replace_field(field: &[char], vars: Vars, depth: u8) -> Result<String, FormatError> {
    let mut at = 0;
    let mut in_index = false;
    while at < field.len() {
        match field[at] {
            '[' => in_index = true,
            ']' => in_index = false,
            '{' => return invalid("unexpected '{' in field name"),
            '!' | ':' if !in_index => break,
            _ => {}
        }
        at += 1;
    }
    let name: String = field[..at].iter().collect();
    let mut conversion = None;
    let mut spec: &[char] = &[];
    if at < field.len() {
        if field[at] == '!' {
            let Some(&code) = field.get(at + 1) else {
                return invalid("end of string while looking for conversion specifier");
            };
            conversion = Some(code);
            at += 2;
            if at < field.len() {
                if field[at] != ':' {
                    return invalid("expected ':' after conversion specifier");
                }
                spec = &field[at + 1..];
            }
        } else {
            spec = &field[at + 1..];
        }
    }

    let value = lookup(&name, vars)?;
    let value = match conversion {
        None | Some('s') => value,
        Some('r') => py_repr(&value),
        Some('a') => py_ascii(&value),
        Some(code) => return invalid(format!("Unknown conversion specifier {code}")),
    };
    let spec = if spec.contains(&'{') {
        expand(spec, vars, depth - 1)?
    } else {
        spec.iter().collect()
    };
    format_str(&value, &spec)
}

/// A field name against string keyword arguments: `name`, `name[3]`, `name.attr`.
fn lookup(name: &str, vars: Vars) -> Result<String, FormatError> {
    let split = name.find(['.', '[']).unwrap_or(name.len());
    let (first, mut rest) = name.split_at(split);
    if first.is_empty() || first.bytes().all(|b| b.is_ascii_digit()) {
        let index = if first.is_empty() { "0" } else { first };
        return invalid(format!(
            "Replacement index {index} out of range for positional args tuple"
        ));
    }
    let Some((_, value)) = vars.iter().find(|(key, _)| *key == first) else {
        return Err(FormatError::Key(first.to_owned()));
    };
    let mut value = (*value).to_owned();
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix('.') {
            let end = after.find(['.', '[']).unwrap_or(after.len());
            let attr = &after[..end];
            if attr.is_empty() {
                return invalid("Empty attribute in format string");
            }
            return invalid(format!("'str' object has no attribute '{attr}'"));
        }
        let after = &rest[1..];
        let Some(end) = after.find(']') else {
            return invalid("Missing ']' in format string");
        };
        let index = &after[..end];
        if index.is_empty() || !index.bytes().all(|b| b.is_ascii_digit()) {
            return invalid("string indices must be integers, not 'str'");
        }
        let Some(ch) = index
            .parse::<usize>()
            .ok()
            .and_then(|n| value.chars().nth(n))
        else {
            return invalid("string index out of range");
        };
        value = ch.to_string();
        rest = &after[end + 1..];
        if !rest.is_empty() && !rest.starts_with(['.', '[']) {
            return invalid("Only '.' or '[' may follow ']' in format field specifier");
        }
    }
    Ok(value)
}

/// `format(value, spec)` for a `str`: `[[fill]align][0][width][.precision][s]`.
fn format_str(value: &str, spec: &str) -> Result<String, FormatError> {
    if spec.is_empty() {
        return Ok(value.to_owned());
    }
    let s: Vec<char> = spec.chars().collect();
    let is_align = |c: char| matches!(c, '<' | '>' | '^' | '=');
    let mut i = 0;
    let (mut fill, mut align) = (' ', None);
    if s.len() >= 2 && is_align(s[1]) {
        fill = s[0];
        align = Some(s[1]);
        i = 2;
    } else if !s.is_empty() && is_align(s[0]) {
        align = Some(s[0]);
        i = 1;
    }
    if matches!(s.get(i), Some('+' | '-' | ' ')) {
        return invalid("Sign not allowed in string format specifier");
    }
    if s.get(i) == Some(&'#') {
        return invalid("Alternate form (#) not allowed in string format specifier");
    }
    if s.get(i) == Some(&'0') {
        if align.is_none() {
            fill = '0';
            align = Some('<');
        }
        i += 1;
    }
    let digits = |i: &mut usize| -> Option<usize> {
        let start = *i;
        while s.get(*i).is_some_and(char::is_ascii_digit) {
            *i += 1;
        }
        s[start..*i].iter().collect::<String>().parse().ok()
    };
    let width = digits(&mut i);
    if matches!(s.get(i), Some(',' | '_')) {
        return invalid(format!("Cannot specify '{}' with 's'.", s[i]));
    }
    let mut precision = None;
    if s.get(i) == Some(&'.') {
        i += 1;
        precision = digits(&mut i);
        if precision.is_none() {
            return invalid("Format specifier missing precision");
        }
    }
    if i + 1 < s.len() {
        return invalid(format!(
            "Invalid format specifier '{spec}' for object of type 'str'"
        ));
    }
    if let Some(&code) = s.get(i)
        && code != 's'
    {
        return invalid(format!(
            "Unknown format code '{code}' for object of type 'str'"
        ));
    }
    if align == Some('=') {
        return invalid("'=' alignment not allowed in string format specifier");
    }

    let text: String = match precision {
        Some(limit) => value.chars().take(limit).collect(),
        None => value.to_owned(),
    };
    let pad = width.unwrap_or(0).saturating_sub(text.chars().count());
    let (left, right) = match align.unwrap_or('<') {
        '>' => (pad, 0),
        '^' => (pad / 2, pad - pad / 2),
        _ => (0, pad),
    };
    let fill = fill.to_string();
    Ok(format!("{}{text}{}", fill.repeat(left), fill.repeat(right)))
}

/// `repr(str)`.
pub fn py_repr(text: &str) -> String {
    let quote = if text.contains('\'') && !text.contains('"') {
        '"'
    } else {
        '\''
    };
    let mut out = String::from(quote);
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c == quote => {
                out.push('\\');
                out.push(c);
            }
            c if c < ' ' || c == '\x7f' => {
                let _ = write!(out, "\\x{:02x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push(quote);
    out
}

/// `ascii(str)`: `repr` with everything outside ASCII escaped.
fn py_ascii(text: &str) -> String {
    let mut out = String::new();
    for c in py_repr(text).chars() {
        match c as u32 {
            0..=0x7f => out.push(c),
            n @ 0x80..=0xff => {
                let _ = write!(out, "\\x{n:02x}");
            }
            n @ 0x100..=0xffff => {
                let _ = write!(out, "\\u{n:04x}");
            }
            n => {
                let _ = write!(out, "\\U{n:08x}");
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const VARS: Vars<'static> = &[
        ("project_name", "demo"),
        ("source_code", "a\nb"),
        ("instruction", ""),
    ];

    fn f(template: &str) -> Result<String, FormatError> {
        format(template, VARS)
    }

    #[test]
    fn fields_and_escaped_braces() {
        assert_eq!(f("{project_name}: {{x}}").unwrap(), "demo: {x}");
        assert_eq!(f("a}}b{{").unwrap(), "a}b{");
    }

    #[test]
    fn unknown_names_and_positions_fail_differently() {
        assert_eq!(f("{nope}"), Err(FormatError::Key("nope".into())));
        assert_eq!(
            f("{}"),
            Err(FormatError::Value(
                "Replacement index 0 out of range for positional args tuple".into()
            ))
        );
    }

    #[test]
    fn malformed_templates_are_value_errors() {
        assert_eq!(
            f("}"),
            Err(FormatError::Value(
                "Single '}' encountered in format string".into()
            ))
        );
        assert_eq!(
            f("{"),
            Err(FormatError::Value(
                "Single '{' encountered in format string".into()
            ))
        );
        assert_eq!(
            f("{project_name"),
            Err(FormatError::Value(
                "expected '}' before end of string".into()
            ))
        );
    }

    #[test]
    fn specs_pad_truncate_and_align() {
        assert_eq!(f("{project_name:>6}").unwrap(), "  demo");
        assert_eq!(f("{project_name:*^8}").unwrap(), "**demo**");
        assert_eq!(f("{project_name:.2}").unwrap(), "de");
        assert_eq!(f("{project_name:06}").unwrap(), "demo00");
        assert_eq!(f("{project_name:s}").unwrap(), "demo");
        assert!(f("{project_name:d}").is_err());
        assert!(f("{project_name:+}").is_err());
    }

    #[test]
    fn nested_specs_expand() {
        let vars: Vars = &[("v", "ab"), ("w", "5")];
        assert_eq!(format("{v:>{w}}", vars).unwrap(), "   ab");
    }

    #[test]
    fn conversions_and_indexing() {
        assert_eq!(f("{source_code!r}").unwrap(), "'a\\nb'");
        assert_eq!(f("{project_name[1]}").unwrap(), "e");
        assert!(f("{project_name.upper}").is_err());
        assert_eq!(py_repr("it's"), "\"it's\"");
        assert_eq!(py_ascii("é"), "'\\xe9'");
    }

    #[test]
    fn default_snapshot_layout() {
        let vars: Vars = &[
            ("project_name", "a&b"),
            ("tree_structure", "└── x.py"),
            ("source_code", "<file path=\"x.py\">\n1\n</file>"),
            ("instruction", "why </file>"),
        ];
        let text = get_engine(Path::new("/nonexistent"))
            .render("snapshot", vars)
            .unwrap();
        assert_eq!(
            text,
            "<project_snapshot project=\"a&amp;b\">\n\
             <user_instruction>\nwhy &lt;/file>\n</user_instruction>\n\
             <project_structure>\n└── x.py\n</project_structure>\n\
             <source_code>\n<file path=\"x.py\">\n1\n</file>\n</source_code>\n\
             </project_snapshot>"
        );
    }

    #[test]
    fn an_empty_project_name_leaves_a_blank_head() {
        let vars: Vars = &[
            ("project_name", ""),
            ("tree_structure", "t"),
            ("source_code", "s"),
        ];
        let text = get_engine(Path::new("/nonexistent"))
            .render("snapshot", vars)
            .unwrap();
        assert!(text.starts_with("<project_snapshot >\n"));
    }

    #[test]
    fn diff_semdiff_and_digest_layouts() {
        let engine = get_engine(Path::new("/nonexistent"));
        let diff = engine
            .render(
                "diff",
                &[
                    ("project_name", "p"),
                    ("context_message", "since mark"),
                    ("diff_content", "a </changes> b"),
                    ("instruction", ""),
                ],
            )
            .unwrap();
        assert_eq!(
            diff,
            "<changes project=\"p\" source=\"since mark\">\na &lt;/changes> b\n</changes>"
        );
        let digest = engine
            .render(
                "digest",
                &[
                    ("project_name", "p"),
                    ("tree_structure", "t"),
                    ("digest_content", "d"),
                    ("instruction", ""),
                ],
            )
            .unwrap();
        assert!(digest.starts_with(
            "<code_digest project=\"p\" contents=\"signatures and docstrings, no bodies\">\n"
        ));
        let semdiff = engine
            .render(
                "semdiff",
                &[
                    ("project_name", "p"),
                    ("context_message", "m"),
                    ("semdiff_content", "s"),
                    ("instruction", ""),
                ],
            )
            .unwrap();
        assert_eq!(
            semdiff,
            "<semantic_diff project=\"p\" source=\"m\">\ns\n</semantic_diff>"
        );
    }

    #[test]
    fn an_override_replaces_the_layout_and_escapes_only_the_instruction() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join(".codigest")).unwrap();
        fs::write(
            dir.path().join(".codigest").join("prompts.toml"),
            "[prompts]\nsnapshot = \"{project_name}|{instruction}|{source_code}\"\nbad = \"{x}\"\n",
        )
        .unwrap();
        let engine = get_engine(dir.path());
        let vars: Vars = &[
            ("project_name", "p"),
            ("source_code", "</file>"),
            ("instruction", "</file>"),
        ];
        assert_eq!(
            engine.render("snapshot", vars).unwrap(),
            "p|&lt;/file>|</file>"
        );
        assert_eq!(
            engine.render("bad", vars).unwrap(),
            "Error rendering template 'bad': Missing argument 'x'"
        );
    }

    #[test]
    fn a_broken_template_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join(".codigest")).unwrap();
        fs::write(
            dir.path().join(".codigest").join("prompts.toml"),
            "[prompts]\nsnapshot = \"oops }\"\n",
        )
        .unwrap();
        let err = get_engine(dir.path()).render("snapshot", &[]).unwrap_err();
        assert_eq!(err.to_string(), "Single '}' encountered in format string");
    }
}
