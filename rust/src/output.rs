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
            if use_osc52() {
                match send_osc52(&content) {
                    Ok(()) => self.log(&format!("Sent {item} to the terminal clipboard (OSC 52)")),
                    Err(_) => {
                        self.log("No terminal to send the clipboard to; use -s to print instead")
                    }
                }
            } else {
                match copy_to_clipboard(&content) {
                    Ok(()) => self.log(&format!("Copied {item} to clipboard")),
                    Err(_) => self.log("Clipboard unavailable; use -s to print instead"),
                }
            }
        }
        if let Some(path) = &self.file {
            fs::write(path, text_mode(&format!("{content}\n")))?;
            self.log(&format!("Wrote {item} to {}", path.display()));
        }
        if self.stdout {
            let mut out = io::stdout().lock();
            out.write_all(text_mode(&format!("{content}\n")).as_bytes())?;
        } else {
            summary(self);
        }
        Ok(())
    }
}

/// What Python's text-mode stdout and `write_text` produce: on Windows every `\n` becomes
/// `\r\n` (a `\r\n` already in the content becomes `\r\r\n`, as it does there).
pub fn text_mode(text: &str) -> String {
    if cfg!(windows) {
        text.replace('\n', "\r\n")
    } else {
        text.to_string()
    }
}

/// Over SSH the system clipboard is the remote machine's, not the one of the person typing.
/// `CODIGEST_CLIPBOARD=osc52` or `=system` overrides the guess, e.g. inside a container
/// reached through `docker exec`, where no SSH variable is set.
fn use_osc52() -> bool {
    match std::env::var("CODIGEST_CLIPBOARD").as_deref() {
        Ok("osc52") => true,
        Ok("system") => false,
        _ => ["SSH_CONNECTION", "SSH_CLIENT", "SSH_TTY"]
            .iter()
            .any(|name| std::env::var_os(name).is_some()),
    }
}

/// The sequence that asks the terminal to put `text` on its own clipboard.
pub fn osc52(text: &str) -> String {
    format!("\x1b]52;c;{}\x07", base64(text.as_bytes()))
}

/// Writes to the terminal itself, so the sequence reaches it even when stdout is a pipe.
fn send_osc52(text: &str) -> anyhow::Result<()> {
    let tty = if cfg!(windows) { "CONOUT$" } else { "/dev/tty" };
    let mut out = fs::OpenOptions::new().write(true).open(tty)?;
    out.write_all(osc52(text).as_bytes())?;
    out.flush()?;
    Ok(())
}

fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, &b)| n | u32::from(b) << (16 - 8 * i));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(TABLE[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

// Termux has no window system for arboard; its own helper owns the Android clipboard.
#[cfg(target_os = "android")]
fn copy_to_clipboard(text: &str) -> anyhow::Result<()> {
    pipe_to("termux-clipboard-set", &[], text)
}

#[cfg(not(target_os = "android"))]
fn copy_to_clipboard(text: &str) -> anyhow::Result<()> {
    // A linux-gnu binary run inside Termux still reports the Android environment this way.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn osc52_carries_the_text_in_base64() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(osc52("한글 ok\n"), "\x1b]52;c;7ZWc6riAIG9rCg==\x07");
    }
}
