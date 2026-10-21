mod common;

use common::{Fixture, assert_parity, lf, parity_enabled};

#[test]
fn tree_prints_the_ascii_tree() {
    let fx = Fixture::new();
    fx.write("a.py", "");
    fx.write("src/b.py", "");
    fx.write("src/pkg/c.py", "");
    let run = fx.rust(&["tree"]);
    assert_eq!(
        run.stdout,
        "├── a.py\n└── src\n    ├── b.py\n    └── pkg\n        └── c.py\n"
    );
    let run = fx.rust(&["tree", "-L", "1"]);
    assert_eq!(run.stdout, "├── a.py\n└── src/ (2 files)\n");
}

#[test]
fn tree_of_a_folder_with_nothing_to_show_warns_and_succeeds() {
    let fx = Fixture::new();
    let run = fx.rust(&["tree"]);
    assert_eq!(run.code, 0);
    assert_eq!(run.stdout, "");
    assert!(run.stderr.contains("[Warning] No matching files found."));
}

#[test]
fn tree_rejects_depth_zero_and_resolve() {
    let fx = Fixture::new();
    assert_eq!(fx.rust(&["tree", "-L", "0"]).code, 2);
    assert_eq!(fx.rust(&["tree", "-r"]).code, 2);
}

#[test]
fn init_creates_the_config_hides_the_folder_and_updates_gitignore() {
    let fx = Fixture::new();
    let run = fx.rust(&["init"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert!(run.stdout.contains("  Created .codigest/config.toml\n"));
    assert!(
        fx.read(".codigest/config.toml")
            .starts_with("# Codigest Configuration File\n")
    );
    assert_eq!(fx.read(".codigest/.gitignore"), "*\n");
    assert_eq!(fx.read(".gitignore"), "# Git Ignore\n.codigest/\n");

    let again = fx.rust(&["init"]);
    assert!(
        again
            .stdout
            .contains("  Kept .codigest/config.toml (--force overwrites it)\n")
    );
    assert!(!again.stdout.contains("Added"));
    assert!(fx.rust(&["init", "--force"]).stdout.contains("Created"));
}

#[test]
fn init_appends_to_an_existing_gitignore_once() {
    let fx = Fixture::new();
    fx.write(".gitignore", "*.log");
    assert!(
        fx.rust(&["init"])
            .stdout
            .contains("  Added .codigest/ to .gitignore\n")
    );
    assert_eq!(fx.read(".gitignore"), "*.log\n.codigest/\n");
    assert!(!fx.rust(&["init"]).stdout.contains("Added"));
}

#[test]
fn init_needs_an_existing_folder() {
    let fx = Fixture::new();
    assert_eq!(fx.rust(&["init", "missing"]).code, 2);
}

#[test]
fn clean_removes_the_folder_after_confirmation() {
    let fx = Fixture::new();
    fx.rust(&["init"]);
    let run = fx.run(common::RUST, &["clean"], Some("n\n"));
    assert_eq!(run.code, 0);
    assert!(
        run.stdout
            .contains("Are you sure you want to remove .codigest? [y/N]: ")
    );
    assert!(run.stdout.contains("Aborted."));
    assert!(fx.root.join(".codigest").exists());

    let run = fx.run(common::RUST, &["clean"], Some("y\n"));
    assert!(run.stdout.contains("Removed "));
    assert!(!fx.root.join(".codigest").exists());

    let run = fx.rust(&["clean", "-y"]);
    assert!(run.stdout.contains("No .codigest directory found in "));
    assert_eq!(run.code, 0);
}

#[test]
fn clean_without_an_answer_aborts() {
    let fx = Fixture::new();
    fx.rust(&["init"]);
    let run = fx.rust(&["clean"]);
    assert_eq!(run.code, 1);
    assert!(run.stderr.contains("Aborted!"));
    assert!(fx.root.join(".codigest").exists());
}

/// Output that names the project folder differs by temp parent only.
fn named(text: &str, fx: &Fixture) -> String {
    let root = fx.root.display().to_string();
    let real = common::real_path(&fx.root);
    lf(text).replace(&real, "<ROOT>").replace(&root, "<ROOT>")
}

#[test]
fn parity_tree() {
    if !parity_enabled() {
        return;
    }
    let fx = Fixture::sample();
    for args in [
        vec!["tree"],
        vec!["tree", "-a"],
        vec!["tree", "-L", "1"],
        vec!["tree", "-L", "2"],
        vec!["tree", "src"],
        vec!["tree", "src", "-L", "1"],
        vec!["tree", "missing"],
    ] {
        assert_parity(&fx, &args);
    }
    fx.config("[filter]\nextensions = [\".md\"]\n");
    assert_parity(&fx, &["tree"]);
    assert_parity(&fx, &["tree", "-a", "-L", "1"]);
}

#[test]
fn parity_init_and_clean() {
    if !parity_enabled() {
        return;
    }
    for steps in [
        vec![
            vec!["init"],
            vec!["init"],
            vec!["init", "--force"],
            vec!["clean", "-y"],
            vec!["clean", "-y"],
        ],
        vec![vec!["init", "-f"], vec!["clean", "--force"]],
    ] {
        let (py, rs) = (Fixture::new(), Fixture::new());
        for args in steps {
            let a = py.python(&args);
            let b = rs.rust(&args);
            assert_eq!(
                named(&a.stdout, &py),
                named(&b.stdout, &rs),
                "stdout differs for {args:?}"
            );
            assert_eq!(a.code, b.code, "exit code differs for {args:?}");
        }
    }
    let (py, rs) = (Fixture::new(), Fixture::new());
    for fx in [&py, &rs] {
        fx.write(".gitignore", "*.log\n");
    }
    let a = py.python(&["init"]);
    let b = rs.rust(&["init"]);
    assert_eq!(named(&a.stdout, &py), named(&b.stdout, &rs));
    for file in [
        ".codigest/config.toml",
        ".codigest/.gitignore",
        ".gitignore",
    ] {
        assert_eq!(lf(&py.read(file)), rs.read(file), "{file} differs");
    }
    let a = py.run("cdg", &["clean"], Some("y\n"));
    let b = rs.run(common::RUST, &["clean"], Some("y\n"));
    assert_eq!(named(&a.stdout, &py), named(&b.stdout, &rs));
    let a = py.run("cdg", &["clean"], Some("n\n"));
    let b = rs.run(common::RUST, &["clean"], Some("n\n"));
    assert_eq!(named(&a.stdout, &py), named(&b.stdout, &rs));
}
