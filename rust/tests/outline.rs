//! Outlines of the fixtures in `tests/fixtures/outline`, compared with the `.expected` file
//! beside each. Every fixture has nested items, doc comments, generics and an item after a
//! syntax error.

use std::fs;
use std::path::Path;

use codigest::outline;

fn check(name: &str) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/outline");
    let source = fs::read_to_string(dir.join(name)).expect("fixture");
    let extension = name.rsplit('.').next().expect("extension");
    let actual = outline::summarize(extension, &source);
    let expected_path = dir.join(format!("{name}.expected"));
    if std::env::var_os("UPDATE_FIXTURES").is_some() {
        fs::write(&expected_path, format!("{actual}\n")).expect("write expected");
    }
    let expected = fs::read_to_string(&expected_path).expect("expected output");
    assert_eq!(actual, expected.trim_end_matches('\n'), "{name}");
}

#[test]
fn rust_outline() {
    check("sample.rs");
}

#[test]
fn typescript_outline() {
    check("sample.ts");
}

#[test]
fn tsx_outline() {
    check("sample.tsx");
}

#[test]
fn javascript_outline() {
    check("sample.js");
}

#[test]
fn go_outline() {
    check("sample.go");
}
