//! Colour for status lines, only when they reach a terminal and NO_COLOR is unset.

use std::io::{self, IsTerminal};
use std::sync::LazyLock;

static ENABLED: LazyLock<bool> = LazyLock::new(|| {
    std::env::var_os("NO_COLOR").is_none()
        && io::stderr().is_terminal()
        && io::stdout().is_terminal()
});

fn paint(code: &str, text: &str) -> String {
    if *ENABLED {
        format!("\x1b[{code}m{text}\x1b[0m")
    } else {
        text.to_string()
    }
}

pub fn dim(text: &str) -> String {
    paint("2", text)
}

pub fn red(text: &str) -> String {
    paint("31", text)
}

pub fn green(text: &str) -> String {
    paint("32", text)
}

pub fn yellow(text: &str) -> String {
    paint("33", text)
}

pub fn bold(text: &str) -> String {
    paint("1", text)
}

pub fn cyan(text: &str) -> String {
    paint("1;36", text)
}
