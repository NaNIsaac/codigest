//! `term install` / `term uninstall`, always against a temporary HOME and a PATH holding
//! nothing but a stand-in `pwsh`, so no real startup file can be touched.

#[path = "capture_common/mod.rs"]
mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as Process;

use common::{posix_sh, python, require_parity, rust, rust_exe};
use regex::Regex;

#[cfg(unix)]
fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) {}

struct Sandbox {
    _dir: tempfile::TempDir,
    home: PathBuf,
    data: PathBuf,
    bin: PathBuf,
    profile: PathBuf,
}

impl Sandbox {
    /// `with_pwsh`: a stand-in `pwsh` that reports a profile path inside the sandbox.
    fn new(with_pwsh: bool) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let root = PathBuf::from(root.to_string_lossy().trim_start_matches(r"\\?\"));
        let (home, data, bin) = (root.join("home"), root.join("data"), root.join("bin"));
        let profile = home
            .join("Documents")
            .join("PowerShell")
            .join("profile.ps1");
        fs::create_dir_all(&home).unwrap();
        fs::create_dir_all(&bin).unwrap();
        if with_pwsh {
            if cfg!(windows) {
                fs::write(
                    bin.join("pwsh.cmd"),
                    format!("@echo off\r\necho {}\r\n", profile.display()),
                )
                .unwrap();
            } else {
                let script = bin.join("pwsh");
                fs::write(
                    &script,
                    format!(
                        "#!{}
echo '{}'
",
                        posix_sh().display(),
                        profile.display()
                    ),
                )
                .unwrap();
                make_executable(&script);
            }
        }
        Self {
            _dir: dir,
            home,
            data,
            bin,
            profile,
        }
    }

    fn env(&self, cmd: &mut Process) {
        cmd.env("HOME", &self.home)
            .env("USERPROFILE", &self.home)
            .env("PATH", &self.bin)
            .env_remove("ZDOTDIR");
    }

    fn rust(&self, args: &[&str]) -> std::process::Output {
        let mut cmd = rust(&self.data);
        cmd.env("HOME", &self.home)
            .env("USERPROFILE", &self.home)
            .env("PATH", &self.bin)
            .env_remove("ZDOTDIR");
        cmd.args(args).output().unwrap()
    }

    fn python(&self, args: &[&str]) -> std::process::Output {
        let mut cmd = python(&self.data);
        self.env(&mut cmd);
        cmd.args(args).output().unwrap()
    }

    fn read(&self, path: &Path) -> String {
        String::from_utf8(fs::read(path).unwrap()).unwrap()
    }
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).replace("\r\n", "\n")
}

const BEGIN: &str = "# >>> codigest terminal capture >>>";
const END: &str = "# <<< codigest terminal capture <<<";

#[test]
fn bash_and_zsh_get_one_block_each_and_lose_it_again() {
    let sandbox = Sandbox::new(false);
    let bashrc = sandbox.home.join(".bashrc");
    let zshrc = sandbox.home.join(".zshrc");
    fs::write(&bashrc, "export KEEP=1\n").unwrap();

    let first = sandbox.rust(&["term", "install", "--shell", "all"]);
    assert!(first.status.success(), "{}", text(&first.stderr));
    let after_first = sandbox.read(&bashrc);
    assert!(after_first.starts_with("export KEEP=1\n\n"));
    assert_eq!(after_first.matches(BEGIN).count(), 1);
    assert!(after_first.contains(END));
    assert!(sandbox.read(&zshrc).starts_with(BEGIN));

    // Installing again replaces the block instead of stacking another.
    sandbox.rust(&["term", "install", "--shell", "all"]);
    assert_eq!(sandbox.read(&bashrc), after_first);

    let hook = sandbox.data.join("shell").join("hook.sh");
    assert!(after_first.contains("hook.sh"));
    let script = sandbox.read(&hook);
    assert!(script.contains(&rust_exe().to_string_lossy().into_owned()));

    sandbox.rust(&["term", "uninstall", "--shell", "all"]);
    assert_eq!(sandbox.read(&bashrc), "export KEEP=1\n");
    assert_eq!(sandbox.read(&zshrc), "");
}

#[test]
fn the_powershell_profile_is_found_through_pwsh_and_written_with_a_bom() {
    let sandbox = Sandbox::new(true);
    let output = sandbox.rust(&["term", "install", "--shell", "powershell"]);
    assert!(output.status.success(), "{}", text(&output.stderr));

    let bytes = fs::read(&sandbox.profile).unwrap();
    assert_eq!(&bytes[..3], [0xEF, 0xBB, 0xBF]);
    let profile = String::from_utf8(bytes[3..].to_vec()).unwrap();
    assert!(profile.starts_with(BEGIN));
    assert!(profile.contains("if (Test-Path '"));

    let hook = fs::read_to_string(sandbox.data.join("shell").join("hook.ps1")).unwrap();
    assert!(hook.contains("Test-Path -LiteralPath"));
    assert!(!hook.contains("Get-Command cdg"));

    sandbox.rust(&["term", "uninstall", "--shell", "powershell"]);
    assert_eq!(fs::read(&sandbox.profile).unwrap(), [0xEF, 0xBB, 0xBF]);
}

#[test]
fn a_profile_that_is_not_utf8_is_left_alone() {
    let sandbox = Sandbox::new(false);
    let bashrc = sandbox.home.join(".bashrc");
    fs::write(&bashrc, [0x80, 0x81, b'\n']).unwrap();
    let output = sandbox.rust(&["term", "install", "--shell", "bash"]);
    assert!(text(&output.stdout).contains("not UTF-8"));
    assert_eq!(fs::read(&bashrc).unwrap(), [0x80, 0x81, b'\n']);
}

#[test]
fn uninstall_does_not_create_or_touch_files_without_a_block() {
    let sandbox = Sandbox::new(false);
    let output = sandbox.rust(&["term", "uninstall", "--shell", "bash"]);
    assert!(output.status.success());
    assert!(!sandbox.home.join(".bashrc").exists());
    assert!(!text(&output.stdout).contains("Removed"));
}

#[test]
fn without_a_supported_shell_it_says_so() {
    let sandbox = Sandbox::new(false);
    let output = sandbox.rust(&["term", "install"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(text(&output.stdout).contains("No supported shell found"));
}

#[test]
fn an_unknown_shell_is_a_usage_error() {
    let sandbox = Sandbox::new(false);
    assert_eq!(
        sandbox
            .rust(&["term", "install", "--shell", "fish"])
            .status
            .code(),
        Some(2)
    );
}

#[test]
fn the_installed_posix_hook_parses() {
    let Some(bash) = codigest::runner::which("bash") else {
        return;
    };
    // System32\bash.exe is the WSL launcher, which fails without a Linux distribution.
    if bash
        .to_string_lossy()
        .to_lowercase()
        .contains(r"\windows\system32\")
    {
        return;
    }
    let sandbox = Sandbox::new(false);
    sandbox.rust(&["term", "install", "--shell", "bash"]);
    let hook = sandbox.data.join("shell").join("hook.sh");
    let status = Process::new(bash).arg("-n").arg(&hook).status().unwrap();
    assert!(status.success());
}

// --------------------------------------------------------------------------
// Parity with the Python build
// --------------------------------------------------------------------------

/// The hook path and the sandbox differ by design; everything else must match.
fn scrub(sandbox: &Sandbox, text: &str) -> String {
    let text = text.replace("\r\n", "\n");
    let quoted_hook = Regex::new(r"'[^']*hook\.(ps1|sh)'").unwrap();
    let text = quoted_hook.replace_all(&text, "'<HOOK>.$1'").into_owned();
    let root = sandbox
        .home
        .parent()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let posix_root = root.replace('\\', "/");
    text.replace(&root, "<ROOT>").replace(&posix_root, "<ROOT>")
}

fn profile_files(sandbox: &Sandbox) -> Vec<(String, Vec<u8>)> {
    let mut found = Vec::new();
    for name in [".bashrc", ".zshrc"] {
        if let Ok(bytes) = fs::read(sandbox.home.join(name)) {
            found.push((name.to_string(), bytes));
        }
    }
    if let Ok(bytes) = fs::read(&sandbox.profile) {
        found.push(("profile.ps1".to_string(), bytes));
    }
    found
}

fn scrubbed_profiles(sandbox: &Sandbox) -> Vec<(String, String)> {
    profile_files(sandbox)
        .into_iter()
        .map(|(name, bytes)| {
            let bom = if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
                "<BOM>"
            } else {
                ""
            };
            let body =
                String::from_utf8_lossy(&bytes[if bom.is_empty() { 0 } else { 3 }..]).into_owned();
            (name, format!("{bom}{}", scrub(sandbox, &body)))
        })
        .collect()
}

fn compare_flow(with_pwsh: bool, existing: &[(&str, &[u8])], steps: &[&[&str]]) {
    require_parity();
    let (rs, py) = (Sandbox::new(with_pwsh), Sandbox::new(with_pwsh));
    for sandbox in [&rs, &py] {
        for (name, content) in existing {
            let path = sandbox.home.join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, content).unwrap();
        }
    }
    for step in steps {
        let (a, b) = (rs.rust(step), py.python(step));
        assert_eq!(a.status.code(), b.status.code(), "exit code for {step:?}");
        if step[1] == "install" && b.status.success() {
            // Guards against both builds failing in the same quiet way.
            assert!(
                !b.stdout.is_empty(),
                "the Python build printed nothing for {step:?}"
            );
        }
        assert_eq!(
            scrub(&rs, &text(&a.stdout)),
            scrub(&py, &text(&b.stdout)),
            "stdout for {step:?}"
        );
        assert_eq!(
            scrubbed_profiles(&rs),
            scrubbed_profiles(&py),
            "profiles after {step:?}"
        );
    }
}

#[test]
#[ignore = "needs CODIGEST_PARITY=1 and the Python cdg"]
fn parity_install_and_uninstall_every_shell() {
    compare_flow(
        true,
        &[
            (".bashrc", b"export KEEP=1\n"),
            (".zshrc", b"\n\nalias a=b\n\n"),
        ],
        &[
            &["term", "install", "--shell", "all"],
            &["term", "install", "--shell", "all"],
            &["term", "uninstall", "--shell", "all"],
            &["term", "uninstall", "--shell", "all"],
        ],
    );
}

#[test]
#[ignore = "needs CODIGEST_PARITY=1 and the Python cdg"]
fn parity_single_shells_auto_and_failures() {
    compare_flow(
        true,
        &[(".bashrc", b"# mine\r\nexport A=1\r\n")],
        &[
            &["term", "install", "--shell", "bash"],
            &["term", "install", "--shell", "powershell"],
            &["term", "uninstall", "--shell", "bash"],
            &["term", "uninstall"],
            &["term", "install", "--shell", "fish"],
        ],
    );
    compare_flow(false, &[], &[&["term", "install"], &["term", "uninstall"]]);
    compare_flow(
        false,
        &[(".bashrc", &[0x80, b'\n'])],
        &[&["term", "install", "--shell", "bash"]],
    );
}
