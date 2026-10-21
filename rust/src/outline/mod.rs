//! Outlines for `digest`: one numbered line per definition, bodies left out.

pub mod matlab;
pub mod python;
pub mod unparse;

pub const SUFFIXES: [&str; 3] = ["py", "pyi", "m"];

pub fn summarize(extension: &str, code: &str) -> String {
    match extension {
        "m" => matlab::summarize(code),
        "py" | "pyi" => python::summarize(code),
        _ => String::new(),
    }
}
