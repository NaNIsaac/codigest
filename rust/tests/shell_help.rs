//! `shell-help install` / `uninstall`, always against a temporary HOME and a PATH holding
//! nothing but a stand-in `pwsh`, so no real profile can be touched.

#[path = "capture_common/mod.rs"]
mod common;

use std::fs;
use std::path::PathBuf;
use std::process::Output;

use common::{posix_sh, python, require_parity, rust};
use regex::Regex;

struct Sandbox {
    _dir: tempfile::TempDir,
    root: PathBuf,
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
        let (data, bin) = (root.join("data"), root.join("bin"));
        let profile = root
            .join("Documents")
            .join("PowerShell")
            .join("profile.ps1");
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
                    format!("#!{}\necho '{}'\n", posix_sh().display(), profile.display()),
                )
                .unwrap();
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
                }
            }
        }
        Self {
            _dir: dir,
            root,
            data,
            bin,
            profile,
        }
    }

    fn rust(&self, args: &[&str]) -> Output {
        let mut cmd = rust(&self.data);
        cmd.env("PATH", &self.bin).args(args);
        cmd.output().unwrap()
    }

    fn python(&self, args: &[&str]) -> Output {
        let mut cmd = python(&self.data);
        cmd.env("PATH", &self.bin).args(args);
        cmd.output().unwrap()
    }

    fn profile_bytes(&self) -> Vec<u8> {
        fs::read(&self.profile).unwrap()
    }

    fn set_profile(&self, bytes: &[u8]) {
        fs::create_dir_all(self.profile.parent().unwrap()).unwrap();
        fs::write(&self.profile, bytes).unwrap();
    }

    /// Text with the sandbox path and the help script path (which differs by design:
    /// Python sources its package file, Rust a copy in the data folder) replaced.
    fn normal(&self, text: &str) -> String {
        let help = Regex::new(r"'[^'\r\n]*help\.ps1'").unwrap();
        let text = text
            .replace("\r\n", "\n")
            .replace(&self.root.display().to_string(), "<S>");
        help.replace_all(&text, "'<HELP>'").into_owned()
    }
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).replace("\r\n", "\n")
}

const BEGIN: &str = "# >>> codigest PowerShell help >>>";
const END: &str = "# <<< codigest PowerShell help <<<";

#[test]
fn install_is_idempotent_and_uninstall_keeps_the_rest_of_the_profile() {
    let sandbox = Sandbox::new(true);
    sandbox.set_profile(b"Set-Alias ll Get-ChildItem\n");

    for _ in 0..2 {
        let run = sandbox.rust(&["shell-help", "install"]);
        assert!(run.status.success(), "{}", text(&run.stderr));
        assert!(text(&run.stdout).contains("Installed PowerShell help in "));
        assert!(text(&run.stdout).ends_with("Open a new PowerShell window to apply the change.\n"));
    }
    let bytes = sandbox.profile_bytes();
    assert_eq!(&bytes[..3], [0xEF, 0xBB, 0xBF]);
    let profile = String::from_utf8(bytes[3..].to_vec()).unwrap();
    assert_eq!(profile.matches(BEGIN).count(), 1);
    assert!(profile.starts_with("Set-Alias ll Get-ChildItem\n\n"));
    assert!(profile.contains("help.ps1"));
    assert!(profile.trim_end().ends_with(END));

    let script = fs::read_to_string(sandbox.data.join("shell").join("help.ps1")).unwrap();
    assert!(script.contains("function global:codigest"));

    let run = sandbox.rust(&["shell-help", "uninstall"]);
    assert!(text(&run.stdout).contains("Removed PowerShell help in "));
    let after = sandbox.profile_bytes();
    assert_eq!(&after[3..], b"Set-Alias ll Get-ChildItem\n");

    let again = sandbox.rust(&["shell-help", "uninstall"]);
    assert!(!text(&again.stdout).contains("Removed"));
    assert_eq!(sandbox.profile_bytes(), after);
}

#[test]
fn without_powershell_nothing_is_written() {
    let sandbox = Sandbox::new(false);
    let run = sandbox.rust(&["shell-help", "install"]);
    assert_eq!(run.status.code(), Some(1));
    assert!(text(&run.stdout).contains("PowerShell not found. Install PowerShell first."));
    assert!(!sandbox.data.exists());
}

#[test]
fn a_profile_that_is_not_utf8_is_left_alone() {
    let sandbox = Sandbox::new(true);
    sandbox.set_profile(&[0x80, 0x81, b'\n']);
    let run = sandbox.rust(&["shell-help", "install"]);
    assert!(text(&run.stdout).contains("Skipped non-UTF-8 profile:"));
    assert_eq!(sandbox.profile_bytes(), [0x80, 0x81, b'\n']);
}

#[test]
fn a_subcommand_is_required() {
    let sandbox = Sandbox::new(true);
    assert_eq!(sandbox.rust(&["shell-help"]).status.code(), Some(2));
}

#[test]
#[ignore = "compares with the Python build: CODIGEST_PARITY=1 cargo test -- --ignored"]
fn parity_shell_help() {
    require_parity();
    let cases: [(&str, Option<&[u8]>); 6] = [
        ("absent", None),
        ("plain", Some(b"Set-Alias ll Get-ChildItem\n")),
        (
            "crlf and bom",
            Some(b"\xEF\xBB\xBFWrite-Host 'a'\r\nWrite-Host 'b'\r\n"),
        ),
        ("no trailing newline", Some(b"$x = 1")),
        ("blank", Some(b"\n\n")),
        ("not utf-8", Some(&[0x80, 0x81, b'\n'])),
    ];
    for (name, initial) in cases {
        let (py, rs) = (Sandbox::new(true), Sandbox::new(true));
        for sandbox in [&py, &rs] {
            if let Some(bytes) = initial {
                sandbox.set_profile(bytes);
            }
        }
        for step in [["install"], ["install"], ["uninstall"], ["uninstall"]] {
            let args = ["shell-help", step[0]];
            let (a, b) = (py.python(&args), rs.rust(&args));
            assert_eq!(
                py.normal(&text(&a.stdout)),
                rs.normal(&text(&b.stdout)),
                "{name}: {step:?}"
            );
            assert_eq!(a.status.code(), b.status.code(), "{name}: {step:?}");
            let (pa, pb) = (py.profile_bytes_if_any(), rs.profile_bytes_if_any());
            assert_eq!(
                pa.map(|b| py.normal(&String::from_utf8_lossy(&b))),
                pb.map(|b| rs.normal(&String::from_utf8_lossy(&b))),
                "{name}: profile after {step:?}"
            );
        }
    }
    let (py, rs) = (Sandbox::new(false), Sandbox::new(false));
    let (a, b) = (
        py.python(&["shell-help", "install"]),
        rs.rust(&["shell-help", "install"]),
    );
    assert_eq!(text(&a.stdout), text(&b.stdout));
    assert_eq!(a.status.code(), b.status.code());
    assert_eq!(
        py.python(&["shell-help"]).status.code(),
        rs.rust(&["shell-help"]).status.code()
    );
}

impl Sandbox {
    fn profile_bytes_if_any(&self) -> Option<Vec<u8>> {
        fs::read(&self.profile).ok()
    }
}
