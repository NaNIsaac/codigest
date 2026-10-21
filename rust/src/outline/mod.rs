//! Outlines for `digest`: one numbered line per definition, bodies left out.

mod entry;
pub mod go;
pub mod matlab;
pub mod python;
pub mod rust;
pub mod unparse;

pub const SUFFIXES: [&str; 5] = ["py", "pyi", "m", "rs", "go"];

pub fn summarize(extension: &str, code: &str) -> String {
    match extension {
        "m" => matlab::summarize(code),
        "py" | "pyi" => python::summarize(code),
        "rs" => rust::summarize(code),
        "go" => go::summarize(code),
        _ => String::new(),
    }
}
