//! Times the stages of a digest over the files listed on stdin (one path per line).

use std::io::Read;
use std::time::Instant;

fn main() {
    let mut list = String::new();
    std::io::stdin().read_to_string(&mut list).unwrap();
    let files: Vec<&str> = list.lines().filter(|l| !l.is_empty()).collect();
    let t = Instant::now();
    let codes: Vec<String> = files
        .iter()
        .map(|f| std::fs::read_to_string(f).unwrap_or_default())
        .collect();
    let read = t.elapsed();
    let t = Instant::now();
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_python::LANGUAGE.into())
        .unwrap();
    for c in &codes {
        parser.parse(c, None);
    }
    let parse = t.elapsed();
    let t = Instant::now();
    let out: Vec<String> = codes
        .iter()
        .map(|c| codigest::outline::python::summarize(c))
        .collect();
    let summarize = t.elapsed();
    let joined = out.join("\n");
    let t = Instant::now();
    let masked = codigest::redact::redact_code(&joined);
    let redact = t.elapsed();
    println!(
        "{} files, {} KB out: read {read:?}, parse {parse:?}, summarize {summarize:?}, redact {redact:?}",
        files.len(),
        masked.len() / 1024
    );
}
