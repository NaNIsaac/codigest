"""
Secret Redaction for captured terminal output.

Terminal output routinely contains credentials (API keys, tokens, connection
strings, `env` dumps). Since captured output is meant to be pasted into an LLM,
redaction runs before anything is written to disk -- never at read time.
"""
import re

MASK = "[REDACTED]"

# Commands whose output is credential material by definition.
# Matched against the start of the command line (case-insensitive).
DENY_COMMANDS: list[re.Pattern[str]] = [
    re.compile(p, re.IGNORECASE)
    for p in (
        r"^\s*env\b",
        r"^\s*printenv\b",
        r"^\s*set\s*$",
        r"^\s*export\s*$",
        r"^\s*(get-childitem|gci|ls|dir)\s+env:",
        r"^\s*aws\s+configure\b",
        r"^\s*gh\s+auth\s+token\b",
        r"^\s*docker\s+login\b",
        r"^\s*(cat|type|bat|get-content|gc)\s+.*\.env\b",
        r"^\s*(echo|write-output)\s+.*\$env:",
        r"^\s*security\s+find-generic-password\b",
        r"^\s*kubectl\s+get\s+secret",
    )
]

# (pattern, replacement) -- replacement may use backrefs to keep the key name
# visible while masking the value.
_RULES: list[tuple[re.Pattern[str], str]] = [
    # PEM private keys (multiline, greedy-safe)
    (
        re.compile(
            r"-----BEGIN [A-Z ]*PRIVATE KEY-----.*?-----END [A-Z ]*PRIVATE KEY-----",
            re.DOTALL,
        ),
        f"-----BEGIN PRIVATE KEY-----{MASK}-----END PRIVATE KEY-----",
    ),
    # JWTs
    (
        re.compile(r"\beyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}"),
        MASK,
    ),
    # Vendor-prefixed tokens
    (re.compile(r"\bsk-[A-Za-z0-9_-]{16,}"), MASK),
    (re.compile(r"\bgh[pousr]_[A-Za-z0-9]{20,}"), MASK),
    (re.compile(r"\bgithub_pat_[A-Za-z0-9_]{20,}"), MASK),
    (re.compile(r"\bAKIA[0-9A-Z]{16}\b"), MASK),
    (re.compile(r"\bASIA[0-9A-Z]{16}\b"), MASK),
    (re.compile(r"\bAIza[0-9A-Za-z_-]{20,}"), MASK),
    (re.compile(r"\bxox[baprs]-[A-Za-z0-9-]{10,}"), MASK),
    (re.compile(r"\bglpat-[A-Za-z0-9_-]{16,}"), MASK),
    (re.compile(r"\bnpm_[A-Za-z0-9]{30,}"), MASK),
    # Authorization headers
    (
        re.compile(r"(?i)\b(authorization\s*:\s*)(bearer|basic|token)\s+\S+"),
        rf"\1\2 {MASK}",
    ),
    # URL userinfo: scheme://user:password@host
    (re.compile(r"(?i)\b([a-z][a-z0-9+.-]*://[^:/\s]+:)[^@\s]+@"), rf"\1{MASK}@"),
    # key=value / key: value forms
    (
        re.compile(
            r"(?i)\b((?:api[_-]?key|secret|access[_-]?token|auth[_-]?token|token|"
            r"password|passwd|client[_-]?secret|private[_-]?key)"
            r"\s*[:=]\s*)[\"']?([^\s\"',;]{6,})"
        ),
        rf"\1{MASK}",
    ),
]


def is_denied_command(command: str) -> bool:
    """True if the command's output must never be stored."""
    return any(p.search(command) for p in DENY_COMMANDS)


def redact(text: str) -> str:
    """Masks known secret shapes in `text`."""
    if not text:
        return text
    for pattern, replacement in _RULES:
        text = pattern.sub(replacement, text)
    return text
