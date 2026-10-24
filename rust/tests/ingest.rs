#[path = "capture_common/mod.rs"]
mod common;

use std::fs;
use std::path::Path;

use common::{python, require_parity, rust};
use serde_json::{Value, json};

fn history(home: &Path) -> Vec<Value> {
    let text = fs::read_to_string(home.join("term").join("history.jsonl")).unwrap_or_default();
    text.lines()
        .map(|line| serde_json::from_str(line).expect("a JSON line"))
        .collect()
}

fn hook_json() -> Vec<u8> {
    json!({
        "command": "pytest -q",
        "output": "\u{1b}[31mFAILED\u{1b}[0m\r\npassword=hunter22xyz\n",
        "exit_code": 1,
        "cwd": "/p",
        "shell": "powershell",
        "started_at": "2026-10-17T01:58:14+09:00",
        "duration_ms": 1500,
        "ignored_key": true,
    })
    .to_string()
    .into_bytes()
}

#[test]
fn json_on_stdin_is_normalised_redacted_and_stored() {
    let home = tempfile::tempdir().unwrap();
    rust(home.path())
        .args(["term", "ingest"])
        .write_stdin(hook_json())
        .assert()
        .success();

    let entries = history(home.path());
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["output"], "FAILED\npassword=[REDACTED]");
    assert_eq!(entries[0]["exit_code"], 1);
    assert_eq!(entries[0]["shell"], "powershell");
    assert_eq!(entries[0]["started_at"], "2026-10-17T01:58:14+09:00");
    assert_eq!(entries[0]["duration_ms"], 1500);
    assert!(entries[0].get("ignored_key").is_none());
}

#[test]
fn a_byte_order_mark_before_the_json_is_accepted() {
    let home = tempfile::tempdir().unwrap();
    let mut input = vec![0xEF, 0xBB, 0xBF];
    input.extend(hook_json());
    rust(home.path())
        .args(["term", "ingest"])
        .write_stdin(input)
        .assert()
        .success();
    assert_eq!(history(home.path()).len(), 1);
}

#[test]
fn a_capture_file_becomes_an_entry_and_is_deleted() {
    let home = tempfile::tempdir().unwrap();
    let file = home.path().join("capture");
    fs::write(&file, b"make test\0line one\r\nline two\n").unwrap();

    rust(home.path())
        .args(["term", "ingest", "--exit-code=2", "--shell=bash"])
        .arg(format!("--file={}", file.display()))
        .args([
            "--cwd",
            "/work//proj/",
            "--start=1700000000,5",
            "--end=1700000002.25",
        ])
        .assert()
        .success();

    assert!(!file.exists());
    let entries = history(home.path());
    assert_eq!(entries[0]["command"], "make test");
    assert_eq!(entries[0]["output"], "line one\nline two");
    assert_eq!(entries[0]["exit_code"], 2);
    assert_eq!(entries[0]["shell"], "bash");
    assert_eq!(entries[0]["duration_ms"], 1750);
    let expected_cwd = if cfg!(windows) {
        r"\work\proj"
    } else {
        "/work/proj"
    };
    assert_eq!(entries[0]["cwd"], expected_cwd);
    assert!(
        entries[0]["started_at"]
            .as_str()
            .unwrap()
            .starts_with("2023-11-1")
    );
}

#[test]
fn a_failure_is_logged_for_term_to_surface_and_still_removes_the_file() {
    let home = tempfile::tempdir().unwrap();
    let file = home.path().join("capture");
    fs::write(&file, b"ls\0out").unwrap();

    rust(home.path())
        .args(["term", "ingest", "--start=not-a-number"])
        .arg(format!("--file={}", file.display()))
        .assert()
        .code(1);

    assert!(!file.exists());
    let log = fs::read_to_string(home.path().join("term").join("ingest-errors.log")).unwrap();
    assert!(log.starts_with("--- "));
    assert!(history(home.path()).is_empty());

    rust(home.path())
        .args(["term", "ingest"])
        .write_stdin("not json")
        .assert()
        .code(1);
    rust(home.path())
        .args(["term", "ingest"])
        .write_stdin(r#"{"output": "no command"}"#)
        .assert()
        .code(1);
    let log = fs::read_to_string(home.path().join("term").join("ingest-errors.log")).unwrap();
    assert_eq!(log.matches("--- ").count(), 3);
}

#[test]
fn credential_commands_are_stored_without_output() {
    let home = tempfile::tempdir().unwrap();
    let input = json!({"command": "printenv", "output": "TOKEN=abc", "exit_code": 0}).to_string();
    rust(home.path())
        .args(["term", "ingest"])
        .write_stdin(input)
        .assert()
        .success();
    let entries = history(home.path());
    assert_eq!(entries[0]["output"], "");
    assert_eq!(
        entries[0]["note"],
        "output withheld (command may expose credentials)"
    );
}

#[test]
fn long_output_keeps_its_head_and_tail() {
    let home = tempfile::tempdir().unwrap();
    let output = format!("HEAD\n{}\nTAIL", "x".repeat(100_000));
    rust(home.path())
        .args(["term", "ingest"])
        .write_stdin(json!({"command": "big", "output": output}).to_string())
        .assert()
        .success();
    let entries = history(home.path());
    let stored = entries[0]["output"].as_str().unwrap();
    assert_eq!(entries[0]["truncated"], true);
    assert!(stored.starts_with("HEAD") && stored.ends_with("TAIL"));
    assert!(stored.contains("bytes elided by codigest"));
}

/// Runs one ingest through both builds and compares everything they stored: the timestamps
/// come from the input, and the host name is the same machine's.
fn compare(args: &[String], stdin: &[u8], file: Option<&[u8]>) {
    let mut stored = Vec::new();
    for use_python in [false, true] {
        let home = tempfile::tempdir().unwrap();
        let capture = home.path().join("capture");
        let mut args = args.to_vec();
        if let Some(bytes) = file {
            fs::write(&capture, bytes).unwrap();
            args.push(format!("--file={}", capture.display()));
        }
        let code = if use_python {
            let mut cmd = python(home.path());
            cmd.args(["term", "ingest"]).args(&args);
            run_with_stdin(&mut cmd, stdin)
        } else {
            let output = rust(home.path())
                .args(["term", "ingest"])
                .args(&args)
                .write_stdin(stdin.to_vec())
                .output()
                .unwrap();
            output.status.code()
        };
        let log = home.path().join("term").join("ingest-errors.log");
        stored.push((code, history(home.path()), log.exists(), capture.exists()));
    }
    assert_eq!(stored[0], stored[1], "args {args:?}");
}

fn run_with_stdin(cmd: &mut std::process::Command, stdin: &[u8]) -> Option<i32> {
    use std::io::Write;
    use std::process::Stdio;
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(stdin).unwrap();
    child.wait().unwrap().code()
}

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_string()).collect()
}

#[test]
#[ignore = "needs CODIGEST_PARITY=1 and the Python cdg"]
fn parity_json_from_powershell() {
    require_parity();
    compare(&[], &hook_json(), None);
    let mut with_bom = vec![0xEF, 0xBB, 0xBF];
    with_bom.extend(hook_json());
    compare(&[], &with_bom, None);
}

#[test]
#[ignore = "needs CODIGEST_PARITY=1 and the Python cdg"]
fn parity_capture_file_from_bash() {
    require_parity();
    let file = b"make test\0\x1b[1mbold\x1b[0m\r10%\r100%\r\ndone token=abcdef123456 \n\n";
    compare(
        &args(&[
            "--exit-code=2",
            "--shell=bash",
            "--cwd=D:/work//proj/",
            "--start=1700000000,5",
            "--end=1700000002.25",
        ]),
        b"",
        Some(file),
    );
    compare(
        &args(&["--exit-code=0", "--start=1700000000.999999"]),
        b"",
        Some(b"ls\0ok"),
    );
    compare(&args(&["--shell=zsh"]), b"", Some(b"no output here"));
    compare(&args(&["--end=5"]), b"", Some(b"cmd\0out"));
}

#[test]
#[ignore = "needs CODIGEST_PARITY=1 and the Python cdg"]
fn parity_denied_commands_and_long_output() {
    require_parity();
    compare(
        &[],
        json!({"command": "Get-ChildItem env:", "output": "TOKEN=abc", "exit_code": 0})
            .to_string()
            .as_bytes(),
        None,
    );
    let output = format!("HEAD\n{}\nTAIL", "가나다".repeat(30_000));
    compare(
        &[],
        json!({"command": "big", "output": output, "started_at": "t"})
            .to_string()
            .as_bytes(),
        None,
    );
}

#[test]
#[ignore = "needs CODIGEST_PARITY=1 and the Python cdg"]
fn parity_failures() {
    require_parity();
    compare(&[], b"not json", None);
    compare(&[], br#"{"output": "no command"}"#, None);
    compare(&args(&["--start=oops"]), b"", Some(b"ls\0out"));
}

const SECRET_CASES: &str = include_str!("../../tests/fixtures/capture_secrets.json");
const KEY_LINES: [&str; 3] = [
    "MIIEvQIBADANBgkqhkiG9w0BAQEFAASCBKcwggSjAgEAAoIBAQC7VJTUt9Us8cKj",
    "MZXT6r+lJ8HdS8bvDmY3k1nV0bXnI2e+LwvKtE5j2V8K9uYp4o5Zr1Wq3Xs7Ty0A",
    "9nB2vC4xD6zE8fG0hJ2kL4mN6pQ8rS0tU2vW4xY6zA8bC0dE2fG4hJ6kL8mN0pQ2",
];

fn secret_cases() -> Vec<Value> {
    serde_json::from_str(SECRET_CASES).expect("valid fixture")
}

fn pem() -> String {
    format!(
        "-----BEGIN PRIVATE KEY-----\n{}\n-----END PRIVATE KEY-----\n",
        KEY_LINES.join("\n")
    )
}

fn strings(value: &Value) -> Vec<&str> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect()
}

/// The stored line as written, plus the decoded command and output.
fn stored_text(home: &Path) -> (String, String) {
    let raw = fs::read_to_string(home.join("term").join("history.jsonl")).unwrap();
    let decoded = history(home)
        .iter()
        .map(|e| {
            format!(
                "{}\n{}",
                e["command"].as_str().unwrap(),
                e["output"].as_str().unwrap()
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    (raw, decoded)
}

fn ingest_case(home: &Path, case: &Value, through_file: bool) {
    let (command, output) = (
        case["command"].as_str().unwrap(),
        case["output"].as_str().unwrap(),
    );
    if through_file {
        let file = home.join("capture");
        fs::write(&file, format!("{command}\0{output}")).unwrap();
        rust(home)
            .args(["term", "ingest", "--shell=bash", "--start=1"])
            .arg(format!("--file={}", file.display()))
            .assert()
            .success();
    } else {
        rust(home)
            .args(["term", "ingest"])
            .write_stdin(json!({"command": command, "output": output}).to_string())
            .assert()
            .success();
    }
}

#[test]
fn secrets_never_reach_the_history() {
    for case in secret_cases() {
        for through_file in [false, true] {
            let home = tempfile::tempdir().unwrap();
            ingest_case(home.path(), &case, through_file);
            let (raw, decoded) = stored_text(home.path());
            for secret in strings(&case["secrets"]) {
                assert!(
                    !raw.contains(secret) && !decoded.contains(secret),
                    "{}: {secret}",
                    case["name"]
                );
            }
            // Redaction masks values, not the lines around them.
            for text in strings(&case["keep"]) {
                assert!(
                    decoded.contains(text),
                    "{}: {text}\n{decoded}",
                    case["name"]
                );
            }
        }
    }
}

#[test]
fn a_private_key_cut_by_the_powershell_line_limit_is_masked() {
    let gap = "\n... [12345 lines not kept by the capture hook] ...\n\n";
    let pem = pem();
    let (head, tail) = pem.split_at(pem.len() / 2);
    for output in [
        format!("start\n{head}{gap}more\n"),
        format!("start\n{gap}{tail}"),
    ] {
        let home = tempfile::tempdir().unwrap();
        rust(home.path())
            .args(["term", "ingest"])
            .write_stdin(json!({"command": "Get-Content dump.txt", "output": output}).to_string())
            .assert()
            .success();
        let (_, decoded) = stored_text(home.path());
        for line in KEY_LINES {
            for part in [&line[..24], &line[line.len() - 24..]] {
                assert!(!decoded.contains(part), "{decoded}");
            }
        }
    }
}

/// A progress bar redrawn with \r collapses to one short line, so a key cut where ingest
/// skips the middle of a capture over 4 MB lands in what is stored.
fn cut_key_capture() -> Vec<u8> {
    let keep = 1024 * 1024;
    let pem = pem();
    let progress = b"\rdownloading".repeat(keep);
    let mut data = b"./fetch-and-dump\0".to_vec();
    data.extend_from_slice(&progress[..keep - pem.len() / 2 - 1 - data.len()]);
    data.push(b'\n');
    data.extend_from_slice(pem.as_bytes());
    data.extend_from_slice(&progress);
    data.extend_from_slice(b"\ndone\n");
    data
}

#[test]
fn a_private_key_cut_by_the_bash_hook_limit_is_masked() {
    let home = tempfile::tempdir().unwrap();
    let file = home.path().join("capture");
    fs::write(&file, cut_key_capture()).unwrap();
    rust(home.path())
        .args(["term", "ingest", "--shell=bash", "--start=1"])
        .arg(format!("--file={}", file.display()))
        .assert()
        .success();
    let (_, decoded) = stored_text(home.path());
    assert!(decoded.contains("-----BEGIN PRIVATE KEY-----") && decoded.contains("bytes elided"));
    for line in KEY_LINES {
        assert!(!decoded.contains(&line[..24]), "{decoded}");
    }
}

#[test]
#[ignore = "needs CODIGEST_PARITY=1 and the Python cdg"]
fn parity_secrets_in_realistic_output() {
    require_parity();
    for case in secret_cases() {
        let (command, output) = (
            case["command"].as_str().unwrap(),
            case["output"].as_str().unwrap(),
        );
        compare(
            &[],
            json!({"command": command, "output": output, "started_at": "t"})
                .to_string()
                .as_bytes(),
            None,
        );
        compare(
            &args(&["--shell=bash", "--start=1"]),
            b"",
            Some(format!("{command}\0{output}").as_bytes()),
        );
    }
    let pem = pem();
    let (head, tail) = pem.split_at(pem.len() / 2);
    let gap = "\n... [12345 lines not kept by the capture hook] ...\n\n";
    for output in [
        format!("start\n{head}{gap}more\n"),
        format!("start\n{gap}{tail}"),
    ] {
        compare(
            &[],
            json!({"command": "Get-Content dump.txt", "output": output, "started_at": "t"})
                .to_string()
                .as_bytes(),
            None,
        );
    }
    compare(
        &args(&["--shell=bash", "--start=1"]),
        b"",
        Some(&cut_key_capture()),
    );
}
