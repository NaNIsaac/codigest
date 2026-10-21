mod common;

use common::Fixture;

#[test]
fn every_version_flag_prints_the_version() {
    let fx = Fixture::new();
    for flag in ["-v", "-V", "--version"] {
        let run = fx.rust(&[flag]);
        assert_eq!(run.code, 0, "{flag}");
        assert_eq!(
            run.stdout,
            format!("cdg {}\n", env!("CARGO_PKG_VERSION")),
            "{flag}"
        );
    }
}
