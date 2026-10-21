//! Prints the project files each file imports: `cargo run --example imports -- ROOT FILE...`.

fn main() {
    let mut args = std::env::args().skip(1);
    let root = std::path::PathBuf::from(args.next().expect("root"));
    let mut resolver = codigest::resolver::Resolver::new(&root);
    for path in args {
        let mut deps: Vec<String> = resolver
            .imports_of(std::path::Path::new(&path))
            .iter()
            .map(|p| p.display().to_string())
            .collect();
        deps.sort();
        println!("=== {path}\n{}", deps.join("\n"));
    }
}
