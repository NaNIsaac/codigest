//! Secret masking, ported rule for rule from `core/redact.py`.
//!
//! Captured output is meant to be pasted into a model, so it is masked before it is
//! written to disk; everything Codigest prints is masked again with the code rules.

use std::sync::LazyLock;

use regex::Regex;

pub const MASK: &str = "[REDACTED]";

fn compile(patterns: &[&str]) -> Vec<Regex> {
    patterns
        .iter()
        .map(|p| Regex::new(p).expect("valid pattern"))
        .collect()
}

// Commands whose output is credential material by definition, matched case-insensitively
// against the start of the command line.
static DENY_COMMANDS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    compile(&[
        r"(?i)^\s*env\b",
        r"(?i)^\s*printenv\b",
        r"(?i)^\s*set\s*$",
        r"(?i)^\s*export\s*$",
        r"(?i)^\s*(get-childitem|gci|ls|dir)\s+env:",
        r"(?i)^\s*aws\s+configure\b",
        r"(?i)^\s*gh\s+auth\s+token\b",
        r"(?i)^\s*docker\s+login\b",
        r"(?i)^\s*(cat|type|bat|get-content|gc)\s+.*\.env\b",
        r"(?i)^\s*(echo|write-output)\s+.*\$env:",
        r"(?i)^\s*security\s+find-generic-password\b",
        r"(?i)^\s*kubectl\s+get\s+secret",
    ])
});

static RULES: LazyLock<Vec<(Regex, String)>> = LazyLock::new(|| {
    let token = |p: &str| (Regex::new(p).expect("valid pattern"), MASK.to_string());
    vec![
        (
            Regex::new(
                r"(?s)-----BEGIN [A-Z ]*PRIVATE KEY-----.*?-----END [A-Z ]*PRIVATE KEY-----",
            )
            .expect("valid pattern"),
            format!("-----BEGIN PRIVATE KEY-----{MASK}-----END PRIVATE KEY-----"),
        ),
        token(r"\beyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}"),
        token(r"\bsk-[A-Za-z0-9_-]{16,}"),
        token(r"\bgh[pousr]_[A-Za-z0-9]{20,}"),
        token(r"\bgithub_pat_[A-Za-z0-9_]{20,}"),
        token(r"\bAKIA[0-9A-Z]{16}\b"),
        token(r"\bASIA[0-9A-Z]{16}\b"),
        token(r"\bAIza[0-9A-Za-z_-]{20,}"),
        token(r"\bxox[baprs]-[A-Za-z0-9-]{10,}"),
        token(r"\bglpat-[A-Za-z0-9_-]{16,}"),
        token(r"\bnpm_[A-Za-z0-9]{30,}"),
        (
            Regex::new(r"(?i)\b([a-z][a-z0-9+.-]*://[^:/\s]+:)[^@\s]+@").expect("valid pattern"),
            format!("${{1}}{MASK}@"),
        ),
    ]
});

// Only for terminal output: in source code these would mask ordinary lines such as
// `token = get_token()`.
static CAPTURE_RULES: LazyLock<Vec<(Regex, String)>> = LazyLock::new(|| {
    vec![
        (
            Regex::new(r"(?i)\b(authorization\s*:\s*)(bearer|basic|token)\s+\S+")
                .expect("valid pattern"),
            format!("${{1}}${{2}} {MASK}"),
        ),
        (
            Regex::new(concat!(
                r"(?i)\b((?:api[_-]?key|secret|access[_-]?token|auth[_-]?token|token|",
                r"password|passwd|client[_-]?secret|private[_-]?key)",
                r#"\s*[:=]\s*)["']?([^\s"',;]{6,})"#,
            ))
            .expect("valid pattern"),
            format!("${{1}}{MASK}"),
        ),
    ]
});

static SECRET_FILE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\.env(\..+)?$|\.(pem|key|p12|pfx|keystore|jks)$|^id_(rsa|dsa|ecdsa|ed25519)$|^\.(netrc|pgpass)$")
        .expect("valid pattern")
});

// Templates that document the variables without values.
const SECRET_FILE_OK: [&str; 4] = [".env.example", ".env.sample", ".env.template", ".env.dist"];

pub fn is_denied_command(command: &str) -> bool {
    DENY_COMMANDS.iter().any(|p| p.is_match(command))
}

pub fn is_secret_file(name: &str) -> bool {
    !SECRET_FILE_OK.contains(&name) && SECRET_FILE.is_match(name)
}

fn apply(text: &str, rules: &[(Regex, String)]) -> String {
    rules
        .iter()
        .fold(text.to_string(), |acc, (pattern, replacement)| {
            pattern.replace_all(&acc, replacement.as_str()).into_owned()
        })
}

/// Masks known secret shapes in terminal output.
pub fn redact(text: &str) -> String {
    apply(&apply(text, &RULES), &CAPTURE_RULES)
}

/// Masks only unambiguous secrets (keys, tokens, URL passwords) in source or diffs.
pub fn redact_code(text: &str) -> String {
    apply(text, &RULES)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_tokens_but_keeps_ordinary_code() {
        let line = "key = sk-abcdefghijklmnop1234 and token = get_token()";
        assert_eq!(
            redact_code(line),
            "key = [REDACTED] and token = get_token()"
        );
    }

    #[test]
    fn keeps_the_url_user_and_masks_the_password() {
        assert_eq!(
            redact_code("postgres://admin:hunter22@db:5432"),
            "postgres://admin:[REDACTED]@db:5432"
        );
    }

    #[test]
    fn capture_rules_mask_headers_and_assignments() {
        assert_eq!(
            redact("Authorization: Bearer abc.def password=hunter22"),
            "Authorization: Bearer [REDACTED] password=[REDACTED]"
        );
    }

    #[test]
    fn private_keys_collapse_to_one_marker() {
        let pem = "-----BEGIN RSA PRIVATE KEY-----\nMII\nabc\n-----END RSA PRIVATE KEY-----";
        assert_eq!(
            redact_code(pem),
            "-----BEGIN PRIVATE KEY-----[REDACTED]-----END PRIVATE KEY-----"
        );
    }

    #[test]
    fn denied_commands_and_secret_files() {
        assert!(is_denied_command("  printenv"));
        assert!(is_denied_command("Get-ChildItem env:"));
        assert!(!is_denied_command("environment-check"));
        assert!(
            is_secret_file(".env.local") && is_secret_file("id_ed25519") && is_secret_file("a.pem")
        );
        assert!(!is_secret_file(".env.example") && !is_secret_file("env.py"));
    }
}
