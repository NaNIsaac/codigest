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
        r"(?i)^\s*kubectl\s+config\s+view\b.*--raw\b",
        r"(?i)^\s*az\s+account\s+get-access-token\b",
        r"(?i)^\s*gcloud\s+auth\s+(print-access-token|print-identity-token|application-default\s+print-access-token)\b",
        r"(?i)^\s*op\s+(read|item\s+get)\b",
        r"(?i)^\s*bw\s+get\b",
        r"(?i)^\s*pass\s+show\b",
        r"(?i)^\s*vault\s+(read|kv\s+get)\b",
        r"(?i)^\s*aws\s+sts\s+(get-session-token|assume-role)\b",
        r"(?i)^\s*(cat|type|bat|get-content|gc)\s+.*(\.npmrc|\.pypirc|\.netrc|\.pgpass|\.pem|\.key|id_(rsa|dsa|ecdsa|ed25519))",
        r#"(?i)^\s*(cat|type|bat|get-content|gc)\s+.*(^|[\\/\s"'])credentials(\.json)?["']?(\s|$)"#,
        r"(?i)^\s*(cat|type|bat|get-content|gc)\s+.*credentials\.json\b",
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
        token(r"\b[sr]k_(?:live|test)_[A-Za-z0-9]{16,}"),
        token(r"\bhf_[A-Za-z0-9]{30,}"),
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
                r#"(?im)((?:^|[^A-Za-z0-9])["']?(?:[A-Za-z0-9]+[_-])*"#,
                r"(?:api[_-]?key|secret[_-]?access[_-]?key|secret[_-]?key|secret|",
                r"access[_-]?token|auth[_-]?token|token|password|passwd|passphrase|pass|pwd|",
                r"client[_-]?secret|private[_-]?key)",
                r#"["']?\s*[:=]\s*["']?)([^\s"',;]{6,})"#,
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

/// Drops the hunks of secret files and masks tokens in the rest of a unified diff.
pub fn redact_diff(diff: &str) -> String {
    if diff.is_empty() {
        return String::new();
    }
    let mut starts: Vec<usize> = diff
        .match_indices("diff --git ")
        .map(|(i, _)| i)
        .filter(|&i| i == 0 || diff.as_bytes()[i - 1] == b'\n')
        .collect();
    starts.push(diff.len());
    let mut out = String::from(&diff[..starts[0]]);
    for pair in starts.windows(2) {
        let section = &diff[pair[0]..pair[1]];
        let header = section.split('\n').next().unwrap_or("");
        let name = header
            .split_whitespace()
            .last()
            .unwrap_or("")
            .rsplit('/')
            .next()
            .unwrap_or("");
        if is_secret_file(name) {
            out.push_str(&format!(
                "{header}\n(contents withheld: {name} may contain secrets)\n"
            ));
        } else {
            out.push_str(section);
        }
    }
    redact_code(&out)
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

    const AWS_SECRET: &str = "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY";

    #[test]
    fn key_value_forms_with_quotes_and_prefixed_keys_are_masked_in_output() {
        let aws = format!("AWS_SECRET_ACCESS_KEY={AWS_SECRET}");
        let json = format!(r#""SecretAccessKey": "{AWS_SECRET}""#);
        let lines = [
            (r#"{"password": "hunter2hunter2"}"#, "hunter2hunter2"),
            ("pass: hunter2hunter2", "hunter2hunter2"),
            ("pwd=hunter2hunter2", "hunter2hunter2"),
            (aws.as_str(), AWS_SECRET),
            (json.as_str(), AWS_SECRET),
            ("DB_PASS=hunter2hunter2", "hunter2hunter2"),
            ("MY-TOKEN: hunter2hunter2", "hunter2hunter2"),
            ("'api_key' = 'hunter2hunter2'", "hunter2hunter2"),
            ("export GITHUB_TOKEN=hunter2hunter2", "hunter2hunter2"),
        ];
        for (line, secret) in lines {
            let out = redact(line);
            assert!(
                !out.contains(secret) && out.contains(MASK),
                "{line} -> {out}"
            );
            assert!(
                redact_code(line).contains(secret),
                "{line} is too ambiguous for code"
            );
        }
    }

    #[test]
    fn the_key_value_rule_keeps_the_key_and_the_quotes() {
        assert_eq!(
            redact(r#"{"password": "hunter2hunter2"}"#),
            r#"{"password": "[REDACTED]"}"#
        );
        assert_eq!(redact("DB_PASS=hunter2hunter2"), "DB_PASS=[REDACTED]");
    }

    #[test]
    fn words_that_merely_contain_a_secret_name_are_left_alone() {
        for line in [
            "bypass: xxxxxxxxxxxx",
            "compass: northnorthnorth",
            "passenger: yyyyyyyyyyyy",
            "tokenizer = xxxxxxxxxxxx",
            "password_hash: abcdefghijkl",
            "token_count = 123456789",
            "mypass = abcdefghijkl",
            "pass: short",
        ] {
            assert_eq!(redact(line), line);
        }
    }

    #[test]
    fn vendor_tokens_are_masked_in_code_too() {
        for token in [
            concat!("sk_", "live_51Habcdefghijklmnopqrstuv"),
            concat!("sk_", "test_51Habcdefghijklmnopqrstuv"),
            concat!("rk_", "live_51Habcdefghijklmnopqrstuv"),
            "hf_abcdefghijklmnopqrstuvwxyz0123",
        ] {
            for out in [
                redact(&format!("KEY={token}")),
                redact_code(&format!("KEY={token}")),
            ] {
                assert!(
                    !out.contains(token) && out.contains(MASK),
                    "{token} -> {out}"
                );
            }
        }
        let text = "pk_live_publishable hf_model sk_other_thing task_live_x";
        assert_eq!(redact_code(text), text);
    }

    #[test]
    fn credential_printing_commands_are_denied() {
        for command in [
            "az account get-access-token",
            "az account get-access-token --resource https://x",
            "gcloud auth print-access-token",
            "gcloud auth print-identity-token",
            "gcloud auth application-default print-access-token",
            "op read op://vault/item/field",
            "op item get github --fields password",
            "bw get password github",
            "pass show work/github",
            "vault read secret/app",
            "vault kv get secret/app",
            "kubectl config view --raw",
            "kubectl config view --minify --raw",
            "aws sts get-session-token",
            "aws sts assume-role --role-arn x --role-session-name y",
            "cat ~/.npmrc",
            "type .pypirc",
            "bat ~/.netrc",
            "Get-Content ./server.pem",
            "cat certs/private.key",
            "cat ~/.ssh/id_rsa",
            r"gc $HOME\.ssh\id_ed25519",
            "cat ~/.aws/credentials",
            "cat application_default_credentials.json",
            "  GCLOUD AUTH PRINT-ACCESS-TOKEN",
        ] {
            assert!(is_denied_command(command), "{command}");
        }
    }

    #[test]
    fn nearby_commands_are_not_denied() {
        for command in [
            "az account show",
            "gcloud auth list",
            "op signin",
            "bw status",
            "pass",
            "vault status",
            "kubectl config view",
            "kubectl config get-contexts",
            "aws sts get-caller-identity",
            "cat README.md",
            "cat src/credentials.py",
            "cat keys.py",
            "python -m pytest",
        ] {
            assert!(!is_denied_command(command), "{command}");
        }
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
