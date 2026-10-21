//! `term install` / `term uninstall`: put the capture hook into the shell startup files
//! (PowerShell profiles, `~/.bashrc`, `~/.zshrc`) between marker lines, or take it out.
//!
//! The hook scripts are the ones the Python build ships, embedded here. They are written
//! to the data directory at install time with the lookup of the `cdg` executable pointed
//! at this binary, because a profile has to source a file that exists on disk.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use clap::Args;

use crate::paths;
use crate::runner::which;
use crate::style::{dim, green, red, yellow};
use crate::term::is_space;

const BEGIN: &str = "# >>> codigest terminal capture >>>";
const END: &str = "# <<< codigest terminal capture <<<";

const HOOK_PS1: &str = include_str!("../../../src/codigest/shell/hook.ps1");
const HOOK_SH: &str = include_str!("../../../src/codigest/shell/hook.sh");

const PS1_LOOKUP: &str = "$global:__cdgExe = (Get-Command cdg -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1).Source";
const SH_LOOKUP: &str = "__cdg_exe=$(command -v cdg 2>/dev/null) || return 0";
// The hooks leave `cdg term` and `cdg mcp` lines out of the capture; this binary has
// another name.
const PS1_SELF: &str = r"(cdg|codigest)(\.exe)?\s+(term|mcp)";
const SH_SELF: &str = r"(cdg|codigest)(\.exe)?[[:space:]]+(term|mcp)";

#[derive(Args, Debug)]
pub struct HookArgs {
    /// powershell, bash, zsh or all (default: every one installed)
    #[arg(long, default_value = "auto", value_parser = ["auto", "powershell", "bash", "zsh", "all"])]
    pub shell: String,
}

fn ps_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "''"))
}

fn sh_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

fn patched(source: &str, from: &str, to: &str) -> String {
    assert!(source.contains(from), "hook script changed: {from}");
    source.replacen(from, to, 1)
}

/// The PowerShell hook with its executable lookup replaced by `exe`.
pub fn powershell_hook(exe: &Path) -> String {
    let exe = ps_quote(&exe.to_string_lossy());
    let text = patched(
        HOOK_PS1,
        PS1_LOOKUP,
        &format!("$global:__cdgExe = if (Test-Path -LiteralPath {exe}) {{ {exe} }}"),
    );
    patched(&text, PS1_SELF, &PS1_SELF.replace("(cdg|", "(cdg|cdg-rs|"))
}

/// The bash / zsh hook with its executable lookup replaced by `exe`.
pub fn posix_hook(exe: &Path) -> String {
    let exe = sh_quote(&exe.to_string_lossy());
    let text = patched(
        HOOK_SH,
        SH_LOOKUP,
        &format!("__cdg_exe={exe}; [ -x \"$__cdg_exe\" ] || return 0"),
    );
    patched(&text, SH_SELF, &SH_SELF.replace("(cdg|", "(cdg|cdg-rs|"))
}

/// `C:/x/y` -> `/c/x/y`, the form Git Bash understands.
pub fn to_posix(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    let bytes = text.as_bytes();
    if bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
        return format!(
            "/{}{}",
            char::from(bytes[0]).to_ascii_lowercase(),
            &text[2..]
        );
    }
    text
}

fn hook_dir() -> PathBuf {
    let dir = paths::data_dir().join("shell");
    std::path::absolute(&dir).unwrap_or(dir)
}

/// Asks each installed PowerShell for its CurrentUserCurrentHost profile path.
fn powershell_profiles() -> Vec<PathBuf> {
    let mut profiles = Vec::new();
    for exe in ["pwsh", "powershell"] {
        let Some(found) = which(exe) else { continue };
        let Ok(result) = Command::new(&found)
            .args(["-NoProfile", "-Command", "$PROFILE.CurrentUserCurrentHost"])
            .output()
        else {
            continue;
        };
        let path = String::from_utf8_lossy(&result.stdout).trim().to_string();
        if result.status.success() && !path.is_empty() {
            profiles.push(PathBuf::from(path));
        }
    }
    profiles
}

/// Startup files for the requested shells; `auto` picks the installed ones.
fn targets(shell: &str) -> Vec<PathBuf> {
    let wants =
        |name: &str| shell == name || shell == "all" || (shell == "auto" && which(name).is_some());
    let mut found = Vec::new();
    if ["powershell", "all", "auto"].contains(&shell) {
        found.extend(powershell_profiles());
    }
    if wants("bash") {
        found.push(paths::home().join(".bashrc"));
    }
    if wants("zsh") {
        let base = env::var_os("ZDOTDIR")
            .filter(|v| !v.is_empty())
            .map_or_else(paths::home, PathBuf::from);
        found.push(base.join(".zshrc"));
    }
    found
}

fn is_powershell(target: &Path) -> bool {
    target.extension().is_some_and(|e| e == "ps1")
}

/// Removes the marked block, if both markers are there.
pub fn strip_block(text: &str) -> String {
    let (Some(start), Some(end)) = (text.find(BEGIN), text.find(END)) else {
        return text.to_string();
    };
    let rest = format!(
        "{}{}",
        &text[..start],
        &text[(end + END.len()).min(text.len())..]
    );
    let rest = rest.trim_matches('\n');
    if rest.is_empty() {
        String::new()
    } else {
        format!("{rest}\n")
    }
}

/// The profile text with the capture block added (replacing an older one).
pub fn with_block(text: &str, line: &str) -> String {
    let stripped = strip_block(text);
    let lead = if stripped.trim_matches(is_space).is_empty() {
        String::new()
    } else {
        format!("{}\n\n", stripped.trim_end_matches('\n'))
    };
    format!("{lead}{BEGIN}\n{line}\n{END}\n")
}

fn rewrite_profiles(add: bool, shell: &str) -> anyhow::Result<i32> {
    // Removing from a file that has no block is a no-op, so uninstall checks every shell.
    let selected = targets(if add || shell != "auto" { shell } else { "all" });
    if selected.is_empty() {
        println!(
            "{} No supported shell found (PowerShell, bash, zsh).",
            red("[Error]")
        );
        return Ok(1);
    }

    let dir = hook_dir();
    let ps1_path = dir.join("hook.ps1");
    let sh_path = dir.join("hook.sh");
    if add {
        let exe = env::current_exe()?;
        fs::create_dir_all(&dir)?;
        fs::write(&ps1_path, powershell_hook(&exe))?;
        fs::write(&sh_path, posix_hook(&exe))?;
    }
    let ps1 = ps1_path.to_string_lossy().into_owned();
    let sh = to_posix(&sh_path);

    for target in &selected {
        let ps = is_powershell(target);
        let mut text = String::new();
        if target.exists() {
            let bytes = fs::read(target)?;
            match std::str::from_utf8(&bytes) {
                Ok(decoded) => {
                    text = decoded
                        .strip_prefix('\u{feff}')
                        .unwrap_or(decoded)
                        .to_string()
                }
                Err(_) => {
                    // Rewriting a non-UTF-8 profile would corrupt it.
                    println!(
                        "{} {} (not UTF-8; edit it by hand)",
                        yellow("Skipped"),
                        target.display()
                    );
                    continue;
                }
            }
        }
        let new = if add {
            let line = if ps {
                format!("if (Test-Path {q}) {{ . {q} }}", q = ps_quote(&ps1))
            } else {
                format!("if [ -f {q} ]; then . {q}; fi", q = sh_quote(&sh))
            };
            with_block(&text, &line)
        } else {
            let stripped = strip_block(&text);
            if stripped == text {
                continue;
            }
            stripped
        };
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        // BOM so Windows PowerShell 5.1 reads non-ASCII paths correctly; a BOM would break bash.
        let mut bytes = if ps {
            vec![0xEF, 0xBB, 0xBF]
        } else {
            Vec::new()
        };
        bytes.extend_from_slice(new.as_bytes());
        fs::write(target, bytes)?;
        println!(
            "{} {}",
            green(if add { "Installed" } else { "Removed" }),
            target.display()
        );
    }

    if add {
        println!("Active in new shell windows. Enable in this one now with:");
        if selected.iter().any(|t| is_powershell(t)) {
            println!("  PowerShell:  . '{ps1}'");
        }
        if selected.iter().any(|t| !is_powershell(t)) {
            println!("  bash / zsh:  . '{sh}'");
        }
        println!(
            "{}",
            dim("Skip one command: start it with a space. Disable: set CODIGEST_NOCAP=1")
        );
    }
    Ok(0)
}

pub fn install(args: HookArgs) -> anyhow::Result<i32> {
    rewrite_profiles(true, &args.shell)
}

pub fn uninstall(args: HookArgs) -> anyhow::Result<i32> {
    rewrite_profiles(false, &args.shell)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_block_is_added_once_and_removed_cleanly() {
        let line = "if [ -f 'x' ]; then . 'x'; fi";
        let once = with_block("export A=1\n", line);
        assert_eq!(once, format!("export A=1\n\n{BEGIN}\n{line}\n{END}\n"));
        assert_eq!(with_block(&once, line), once);
        assert_eq!(strip_block(&once), "export A=1\n");
        assert_eq!(strip_block(&with_block("", line)), "");
        assert_eq!(strip_block("no markers"), "no markers");
    }

    #[test]
    fn paths_become_git_bash_paths() {
        assert_eq!(to_posix(Path::new(r"C:\Users\me\x.sh")), "/c/Users/me/x.sh");
        assert_eq!(to_posix(Path::new("/home/me/x.sh")), "/home/me/x.sh");
    }

    #[test]
    fn the_hooks_point_at_this_binary_and_skip_its_own_commands() {
        let exe = Path::new("/opt/it's/cdg-rs");
        let ps = powershell_hook(exe);
        assert!(ps.contains("Test-Path -LiteralPath '/opt/it''s/cdg-rs'"));
        assert!(ps.contains("(cdg|cdg-rs|codigest)"));
        assert!(!ps.contains("Get-Command cdg"));
        let sh = posix_hook(exe);
        assert!(sh.contains("__cdg_exe='/opt/it'\\''s/cdg-rs'; [ -x"));
        assert!(sh.contains("(cdg|cdg-rs|codigest)"));
        assert!(!sh.contains("command -v cdg"));
    }
}
