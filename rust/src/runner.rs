//! Transparent command runner (`core/runner.py`).
//!
//! Executes a command while mirroring its combined stdout/stderr to this process's stdout
//! in real time, so the user's terminal looks unchanged, and returns the captured bytes
//! for storage.
//!
//! stdout and stderr are merged into a single pipe on purpose: the interleaved stream is
//! what the user actually saw, and that ordering is what makes the capture useful as LLM
//! context.

use std::env;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, Instant};

const SHELL_METACHARS: &str = "|&><;$`*?()";
const READ_SIZE: usize = 65536;
/// How long a child gets to exit after Ctrl+C before it is killed.
const INTERRUPT_GRACE: Duration = Duration::from_secs(5);

#[derive(Debug)]
pub struct Captured {
    pub exit_code: i32,
    pub output: String,
    pub duration_ms: u64,
}

#[derive(Debug)]
pub enum RunError {
    /// The program does not exist (exit code 127 in `cdg cap`).
    NotFound,
    Launch(io::Error),
}

/// Renders argv back into the command line the user effectively typed.
pub fn join_command(args: &[String]) -> String {
    if needs_shell(args) {
        args[0].clone()
    } else if cfg!(windows) {
        list2cmdline(args)
    } else {
        shlex_join(args)
    }
}

/// A single argument carrying shell metacharacters is a pipeline the user typed as one
/// string (e.g. `cdg cap "ls | head"`), so it must go through a shell. Multiple args are
/// an already-tokenized argv and are executed directly.
pub fn needs_shell(args: &[String]) -> bool {
    matches!(args, [only] if only.chars().any(|c| SHELL_METACHARS.contains(c)))
}

/// `subprocess.list2cmdline`: the MS C runtime quoting rules.
pub fn list2cmdline(args: &[String]) -> String {
    let mut result = String::new();
    for (i, arg) in args.iter().enumerate() {
        if i > 0 {
            result.push(' ');
        }
        let need_quote = arg.contains(' ') || arg.contains('\t') || arg.is_empty();
        if need_quote {
            result.push('"');
        }
        let mut backslashes = 0usize;
        for c in arg.chars() {
            match c {
                '\\' => backslashes += 1,
                '"' => {
                    // Backslashes before a quote are doubled, and the quote escaped.
                    result.push_str(&"\\".repeat(backslashes * 2));
                    backslashes = 0;
                    result.push_str("\\\"");
                }
                _ => {
                    result.push_str(&"\\".repeat(backslashes));
                    backslashes = 0;
                    result.push(c);
                }
            }
        }
        // Trailing backslashes: doubled when a closing quote follows.
        result.push_str(&"\\".repeat(backslashes));
        if need_quote {
            result.push_str(&"\\".repeat(backslashes));
            result.push('"');
        }
    }
    result
}

/// `shlex.join`.
pub fn shlex_join(args: &[String]) -> String {
    args.iter()
        .map(|a| shlex_quote(a))
        .collect::<Vec<_>>()
        .join(" ")
}

fn shlex_quote(arg: &str) -> String {
    if arg.is_empty() {
        return "''".to_string();
    }
    let safe = |c: char| c.is_ascii_alphanumeric() || "_@%+=:,./-".contains(c);
    if arg.chars().all(safe) {
        return arg.to_string();
    }
    format!("'{}'", arg.replace('\'', "'\"'\"'"))
}

/// `shutil.which`: the first executable `name` resolves to, or None.
pub fn which(name: &str) -> Option<PathBuf> {
    let candidates = |dir: &Path| -> Vec<PathBuf> {
        if cfg!(windows) {
            let pathext = env::var("PATHEXT")
                .ok()
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| ".COM;.EXE;.BAT;.CMD;.VBS;.JS;.WSH;.MSC".to_string());
            let exts: Vec<&str> = pathext.split(';').filter(|e| !e.is_empty()).collect();
            let lower = name.to_lowercase();
            if exts.iter().any(|e| lower.ends_with(&e.to_lowercase())) {
                vec![dir.join(name)]
            } else {
                exts.iter()
                    .map(|e| dir.join(format!("{name}{e}")))
                    .collect()
            }
        } else {
            vec![dir.join(name)]
        }
    };
    let runnable = |path: &Path| path.is_file() && is_executable(path);

    if Path::new(name)
        .parent()
        .is_some_and(|p| !p.as_os_str().is_empty())
    {
        return candidates(Path::new("")).into_iter().find(|p| runnable(p));
    }

    let mut dirs: Vec<PathBuf> = Vec::new();
    // The current directory takes precedence on Windows.
    if cfg!(windows) && env::var_os("NoDefaultCurrentDirectoryInExePath").is_none() {
        dirs.push(PathBuf::from("."));
    }
    if let Some(path) = env::var_os("PATH") {
        dirs.extend(env::split_paths(&path).filter(|p| !p.as_os_str().is_empty()));
    }
    dirs.iter()
        .flat_map(|dir| candidates(dir))
        .find(|p| runnable(p))
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(_path: &Path) -> bool {
    true
}

/// The argv prefix for running a command string through a shell. Honors CODIGEST_SHELL,
/// then prefers the platform's interactive shell.
pub fn resolve_shell() -> Option<Vec<String>> {
    if let Some(shell) = env::var("CODIGEST_SHELL").ok().filter(|s| !s.is_empty()) {
        let stem = Path::new(&shell)
            .file_stem()
            .map(|s| s.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        if cfg!(windows) && (stem == "pwsh" || stem == "powershell") {
            return Some(vec![shell, "-NoProfile".into(), "-Command".into()]);
        }
        return Some(vec![shell, "-c".into()]);
    }

    if cfg!(windows) {
        // Most Codigest users on Windows type into PowerShell, so pipeline semantics
        // should match PowerShell rather than cmd.exe.
        for candidate in ["pwsh", "powershell"] {
            if let Some(found) = which(candidate) {
                return Some(vec![
                    found.to_string_lossy().into_owned(),
                    "-NoProfile".into(),
                    "-Command".into(),
                ]);
            }
        }
        return env::var("COMSPEC")
            .ok()
            .filter(|c| !c.is_empty())
            .map(|comspec| vec![comspec, "/c".into()]);
    }

    let shell = env::var("SHELL")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| which("bash").map(|p| p.to_string_lossy().into_owned()))
        // Termux has no /bin/sh; its sh lives under $PREFIX/bin.
        .or_else(|| which("sh").map(|p| p.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "/bin/sh".to_string());
    Some(vec![shell, "-c".into()])
}

/// Leaves plain words bare, so `-Name` stays a parameter, and single-quotes the rest.
fn powershell_quote(arg: &str) -> String {
    if !arg.is_empty()
        && arg
            .chars()
            .all(|c| c.is_alphanumeric() || "-_./\\:=+~".contains(c))
    {
        return arg.to_string();
    }
    format!("'{}'", arg.replace('\'', "''"))
}

/// PowerShell aliases, cmdlets and functions (`ls`, `Get-ChildItem`) are not executables,
/// so CreateProcess cannot start them. When the shell is PowerShell and knows the name,
/// returns the argv as a command string for it; otherwise None, and the caller reports
/// the program as not found.
pub fn powershell_command(args: &[String], prefix: &[String]) -> Option<String> {
    let stem = Path::new(&prefix[0])
        .file_stem()
        .map(|s| s.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if stem != "pwsh" && stem != "powershell" {
        return None;
    }
    let probe = format!(
        "exit [int](-not (Get-Command -Name {} -ErrorAction Ignore))",
        powershell_quote(&args[0])
    );
    let known = Command::new(&prefix[0])
        .args(&prefix[1..])
        .arg(probe)
        .output()
        .is_ok_and(|out| out.status.success());
    known.then(|| {
        let words: Vec<String> = args.iter().map(|a| powershell_quote(a)).collect();
        format!("& {}", words.join(" "))
    })
}

/// Maps user-supplied arguments onto an argv for the process.
pub fn build_argv(args: &[String]) -> Vec<String> {
    if !needs_shell(args) {
        if cfg!(windows) && !args.is_empty() {
            // CreateProcess finds neither `.cmd` shims (npm, yarn, pnpm) nor relative paths
            // written with forward slashes; which() resolves both the way the user's shell would.
            if let Some(found) = which(&args[0]) {
                let mut argv = vec![found.to_string_lossy().into_owned()];
                argv.extend(args[1..].iter().cloned());
                return argv;
            }
            if let Some(mut prefix) = resolve_shell()
                && let Some(command) = powershell_command(args, &prefix)
            {
                prefix.push(command);
                return prefix;
            }
        }
        return args.to_vec();
    }
    match resolve_shell() {
        Some(mut prefix) => {
            prefix.push(args[0].clone());
            prefix
        }
        None => args.to_vec(),
    }
}

fn exit_code_of(status: std::process::ExitStatus) -> i32 {
    if let Some(code) = status.code() {
        return code;
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = status.signal() {
            return -signal;
        }
    }
    -1
}

/// Runs the command, streaming output through, and returns the exit code, the output and
/// the duration.
///
/// Ctrl+C reaches the child through the terminal as well; this process survives it, gives
/// the child a few seconds to leave, and still returns the partial output, since an
/// interrupted run is often exactly what the user wants to hand to an LLM.
pub fn run(args: &[String]) -> Result<Captured, RunError> {
    run_argv(&build_argv(args))
}

/// [`run`] for an argv that is already final, such as a PowerShell `-EncodedCommand` call.
pub fn run_argv(argv: &[String]) -> Result<Captured, RunError> {
    let started = Instant::now();

    let (mut reader, writer) = io::pipe().map_err(RunError::Launch)?;
    let mut command = Command::new(&argv[0]);
    command.args(&argv[1..]);
    command.stdout(writer.try_clone().map_err(RunError::Launch)?);
    command.stderr(writer);
    let spawned = command.spawn();
    // The parent's copy of the write end must close, or the reader never sees EOF.
    drop(command);
    let mut child = spawned.map_err(|e| match e.kind() {
        io::ErrorKind::NotFound => RunError::NotFound,
        _ => RunError::Launch(e),
    })?;

    let interrupted = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&interrupted);
    // Only the first registration can succeed in a process; later ones keep it.
    let _ = ctrlc::set_handler(move || flag.store(true, Ordering::SeqCst));

    let (tx, rx) = mpsc::channel();
    let pump = thread::spawn(move || {
        let mut chunks: Vec<u8> = Vec::new();
        let mut buf = vec![0u8; READ_SIZE];
        let mut stdout = io::stdout();
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    chunks.extend_from_slice(&buf[..n]);
                    // A closed stdout must not stop the capture.
                    let _ = stdout.write_all(&buf[..n]).and_then(|()| stdout.flush());
                }
            }
        }
        let _ = tx.send(chunks);
    });

    let mut deadline: Option<Instant> = None;
    let mut killed = false;
    let bytes = loop {
        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(bytes) => break bytes,
            Err(mpsc::RecvTimeoutError::Disconnected) => break Vec::new(),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if interrupted.load(Ordering::SeqCst) {
                    let due = *deadline.get_or_insert_with(|| Instant::now() + INTERRUPT_GRACE);
                    if !killed && Instant::now() >= due {
                        let _ = child.kill();
                        killed = true;
                    }
                }
            }
        }
    };
    let _ = pump.join();

    let status = child.wait().map_err(RunError::Launch)?;
    Ok(Captured {
        exit_code: exit_code_of(status),
        output: String::from_utf8_lossy(&bytes).into_owned(),
        duration_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn a_lone_argument_with_metacharacters_needs_a_shell() {
        assert!(needs_shell(&args(&["ls | head"])));
        assert!(needs_shell(&args(&["echo $HOME"])));
        assert!(!needs_shell(&args(&["pytest -q"])));
        assert!(!needs_shell(&args(&["ls", "|", "head"])));
        assert!(!needs_shell(&[]));
    }

    #[test]
    fn joins_like_shlex() {
        assert_eq!(
            shlex_join(&args(&["echo", "a b", "it's", ""])),
            "echo 'a b' 'it'\"'\"'s' ''"
        );
        assert_eq!(
            shlex_join(&args(&["git", "log", "--format=%h", "a/b.c"])),
            "git log --format=%h a/b.c"
        );
    }

    #[test]
    fn joins_like_list2cmdline() {
        let bs = |n: usize| "\\".repeat(n);
        assert_eq!(list2cmdline(&args(&["a", "b c", ""])), "a \"b c\" \"\"");
        assert_eq!(
            list2cmdline(&args(&["say \"hi\""])),
            format!("\"say {}\"hi{}\"\"", bs(1), bs(1))
        );
        // A trailing backslash doubles when a closing quote follows it.
        assert_eq!(
            list2cmdline(&args(&[&format!("C:{}dir x{}", bs(1), bs(1))])),
            format!("\"C:{}dir x{}\"", bs(1), bs(2))
        );
        // Backslashes before a quote double, and the quote is escaped.
        assert_eq!(
            list2cmdline(&args(&[&format!("a{}\"b", bs(2))])),
            format!("a{}\"b", bs(5))
        );
        assert_eq!(
            list2cmdline(&args(&[&format!("a{}b", bs(1))])),
            format!("a{}b", bs(1))
        );
    }

    #[test]
    fn powershell_words_stay_bare_and_the_rest_is_single_quoted() {
        assert_eq!(powershell_quote("-Name"), "-Name");
        assert_eq!(powershell_quote("C:\\dir/a.txt"), "C:\\dir/a.txt");
        assert_eq!(powershell_quote("a b"), "'a b'");
        assert_eq!(powershell_quote("it's"), "'it''s'");
        assert_eq!(powershell_quote("a,b"), "'a,b'");
        assert_eq!(powershell_quote(""), "''");
    }

    #[test]
    fn a_shell_pipeline_is_kept_as_typed() {
        assert_eq!(join_command(&args(&["ls | head"])), "ls | head");
    }

    #[test]
    fn codigest_shell_overrides_the_platform_choice() {
        // Safe to touch the environment here: no other test in this module reads it.
        let _guard = ENV_LOCK.lock().unwrap();
        // SAFETY: serialised by ENV_LOCK; nothing else in this process reads CODIGEST_SHELL.
        unsafe { env::set_var("CODIGEST_SHELL", "/opt/zsh") };
        assert_eq!(resolve_shell(), Some(args(&["/opt/zsh", "-c"])));
        unsafe { env::set_var("CODIGEST_SHELL", "C:\\PS\\pwsh.exe") };
        let expected = if cfg!(windows) {
            args(&["C:\\PS\\pwsh.exe", "-NoProfile", "-Command"])
        } else {
            args(&["C:\\PS\\pwsh.exe", "-c"])
        };
        assert_eq!(resolve_shell(), Some(expected));
        unsafe { env::remove_var("CODIGEST_SHELL") };
    }

    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn merges_stderr_into_the_captured_output() {
        let program = if cfg!(windows) {
            args(&["cmd", "/c", "echo out& echo err 1>&2& exit 3"])
        } else {
            args(&["sh", "-c", "echo out; echo err >&2; exit 3"])
        };
        let captured = run(&program).unwrap();
        assert_eq!(captured.exit_code, 3);
        let lines: Vec<&str> = captured.output.lines().map(str::trim).collect();
        assert_eq!(lines, ["out", "err"]);
    }

    #[test]
    fn a_missing_program_is_reported_as_not_found() {
        assert!(matches!(
            run(&args(&["codigest-no-such-program-xyz"])),
            Err(RunError::NotFound)
        ));
    }
}
