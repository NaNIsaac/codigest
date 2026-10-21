//! Where a command's result goes: the clipboard, a file and/or stdout, with the defaults of
//! the Python build. In a terminal the result is copied; when stdout is a pipe or a file (a
//! script, an agent's shell) it is printed and the clipboard is left alone.

use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

use clap::Args;

use crate::redact::redact_code;

#[derive(Args, Debug, Default, Clone)]
pub struct OutputArgs {
    /// Copy the result to the clipboard [default: copy in a terminal, not when piped]
    #[arg(short = 'c', long, overrides_with = "no_copy")]
    pub copy: bool,
    #[arg(long, overrides_with = "copy", hide = true)]
    pub no_copy: bool,
    /// Also print the result (on by default when piped)
    #[arg(short = 's', long)]
    pub stdout: bool,
    /// Also write the result to this file
    #[arg(short = 'o', long, value_name = "FILE")]
    pub output: Option<PathBuf>,
}

pub struct Output {
    stdout: bool,
    copy: bool,
    file: Option<PathBuf>,
}

impl Output {
    /// `copy_by_default` is false for commands such as `tree` that only copy on request.
    pub fn new(args: &OutputArgs, copy_by_default: bool) -> Self {
        let chosen = if args.copy {
            Some(true)
        } else if args.no_copy {
            Some(false)
        } else {
            None
        };
        let piped = !io::stdout().is_terminal();
        let copy = match chosen {
            Some(choice) => choice,
            None => copy_by_default && !piped,
        };
        Self {
            stdout: args.stdout || piped,
            copy,
            file: args.output.clone(),
        }
    }

    pub fn prints(&self) -> bool {
        self.stdout
    }

    /// Status lines go to stderr when stdout carries the result, so pipes stay clean.
    pub fn log(&self, message: &str) {
        if self.stdout {
            eprintln!("{message}");
        } else {
            println!("{message}");
        }
    }

    /// Delivers `content`. `summary` replaces the content on screen when it is not printed.
    pub fn deliver(
        &self,
        content: &str,
        item: &str,
        summary: impl FnOnce(&Self),
    ) -> anyhow::Result<()> {
        // Every command's context passes through here, so this is the one place that
        // guarantees no unambiguous token leaves.
        let content = redact_code(content);
        if self.copy {
            match copy_to_clipboard(&content) {
                Ok(()) => self.log(&format!("Copied {item} to clipboard")),
                Err(_) => self.log("Clipboard unavailable; use -s to print instead"),
            }
        }
        if let Some(path) = &self.file {
            fs::write(path, format!("{content}\n"))?;
            self.log(&format!("Wrote {item} to {}", path.display()));
        }
        if self.stdout {
            let mut out = io::stdout().lock();
            writeln!(out, "{content}")?;
        } else {
            summary(self);
        }
        Ok(())
    }
}

fn copy_to_clipboard(text: &str) -> anyhow::Result<()> {
    // Termux has no window system for arboard; its own helper owns the Android clipboard.
    if std::env::var_os("TERMUX_VERSION").is_some() {
        return pipe_to("termux-clipboard-set", &[], text);
    }
    match arboard::Clipboard::new().and_then(|mut c| c.set_text(text.to_owned())) {
        Ok(()) => Ok(()),
        Err(err) if cfg!(target_os = "linux") => pipe_to("wl-copy", &[], text)
            .or_else(|_| pipe_to("xclip", &["-selection", "clipboard"], text))
            .map_err(|_| err.into()),
        Err(err) => Err(err.into()),
    }
}

fn pipe_to(program: &str, args: &[&str], text: &str) -> anyhow::Result<()> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()?;
    child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(text.as_bytes())?;
    anyhow::ensure!(child.wait()?.success(), "{program} failed");
    Ok(())
}
