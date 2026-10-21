//! Outlines for `digest`: one numbered line per definition, bodies left out.

mod entry;
pub mod go;
pub mod js;
pub mod matlab;
pub mod python;
pub mod rust;
pub mod unparse;

pub const SUFFIXES: [&str; 11] = [
    "py", "pyi", "m", "rs", "js", "jsx", "mjs", "cjs", "ts", "tsx", "go",
];

pub fn summarize(extension: &str, code: &str) -> String {
    match extension {
        "m" => matlab::summarize(code),
        "py" | "pyi" => python::summarize(code),
        "rs" => rust::summarize(code),
        "js" | "jsx" | "mjs" | "cjs" | "ts" | "tsx" => js::summarize(extension, code),
        "go" => go::summarize(code),
        _ => String::new(),
    }
}
