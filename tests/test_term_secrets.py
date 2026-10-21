"""
Secrets in captured terminal output, end to end: a command goes in the way each shell hook
hands it over, and neither the stored history nor anything `cdg term` prints holds a secret.

The cases live in fixtures/capture_secrets.json, which the Rust tests read too.
"""

import json
from pathlib import Path

import pytest
from typer.testing import CliRunner

from codigest.commands import term
from codigest.core import termstore

CASES = json.loads(
    (Path(__file__).parent / "fixtures" / "capture_secrets.json").read_text(encoding="utf-8")
)
KEY_LINES = [
    "MIIEvQIBADANBgkqhkiG9w0BAQEFAASCBKcwggSjAgEAAoIBAQC7VJTUt9Us8cKj",
    "MZXT6r+lJ8HdS8bvDmY3k1nV0bXnI2e+LwvKtE5j2V8K9uYp4o5Zr1Wq3Xs7Ty0A",
    "9nB2vC4xD6zE8fG0hJ2kL4mN6pQ8rS0tU2vW4xY6zA8bC0dE2fG4hJ6kL8mN0pQ2",
]
PEM = "-----BEGIN PRIVATE KEY-----\n" + "\n".join(KEY_LINES) + "\n-----END PRIVATE KEY-----\n"


def ingest_json(command: str, output: str) -> None:
    """What the PowerShell hook sends: one JSON object on stdin."""
    data = {"command": command, "output": output, "exit_code": 0, "shell": "powershell"}
    result = CliRunner().invoke(term.app, ["ingest"], input=json.dumps(data))
    assert result.exit_code == 0, result.output


def ingest_file(tmp_path: Path, command: str, output: bytes) -> None:
    """What the bash / zsh hook leaves: command, NUL, raw output, in a private file."""
    capture = tmp_path / "capture"
    capture.write_bytes(command.encode() + b"\0" + output)
    result = CliRunner().invoke(
        term.app, ["ingest", f"--file={capture}", "--exit-code=0", "--shell=bash", "--start=1"]
    )
    assert result.exit_code == 0, result.output
    assert not capture.exists()


def everything_kept(tmp_path: Path) -> str:
    """The history file as stored plus every format `cdg term` can deliver."""
    entries = termstore.load_entries()
    rendered = [
        render(entries, include_context=True)
        for render in (termstore.render, termstore.render_markdown, termstore.render_json)
    ]
    stored = termstore.get_history_path().read_bytes().decode("utf-8")
    # JSON escapes quotes and slashes; compare against the decoded values as well.
    decoded = "\n".join(e.command + "\n" + e.output for e in entries)
    return "\n".join([stored, decoded, *rendered])


@pytest.fixture(autouse=True)
def data_home(tmp_path, monkeypatch):
    monkeypatch.setenv("CODIGEST_HOME", str(tmp_path / "data"))


@pytest.mark.parametrize("case", CASES, ids=[c["name"] for c in CASES])
@pytest.mark.parametrize("hook", ["powershell", "bash"])
def test_secrets_never_reach_the_history(tmp_path, case, hook):
    if hook == "powershell":
        ingest_json(case["command"], case["output"])
    else:
        ingest_file(tmp_path, case["command"], case["output"].encode())
    kept = everything_kept(tmp_path)
    for secret in case["secrets"]:
        assert secret not in kept
    # Redaction masks values, not the lines around them.
    [entry] = termstore.load_entries()
    for text in case["keep"]:
        assert text in entry.command + entry.output


def test_a_private_key_cut_by_the_bash_hook_limit_is_masked(tmp_path):
    """
    Over 4 MB of raw output, ingest reads only the first and last megabyte. A progress bar
    redrawn with \\r collapses to one short line, so the cut key lands in what is stored.
    """
    progress = b"\rdownloading" * term.RAW_KEEP
    head = progress[: term.RAW_KEEP - len(PEM) // 2 - 1] + b"\n"
    ingest_file(tmp_path, "./fetch-and-dump", head + PEM.encode() + progress + b"\ndone\n")
    [entry] = termstore.load_entries()
    assert "-----BEGIN PRIVATE KEY-----" in entry.output and "bytes elided" in entry.output
    kept = everything_kept(tmp_path)
    for line in KEY_LINES:
        assert line[:24] not in kept


@pytest.mark.parametrize("keep", ["head", "tail"])
def test_a_private_key_cut_by_the_powershell_line_limit_is_masked(keep):
    """The PowerShell hook keeps the first and last 2,000 lines and marks the gap."""
    gap = "\n... [12345 lines not kept by the capture hook] ...\n\n"
    head, tail = PEM[: len(PEM) // 2], PEM[len(PEM) // 2 :]
    output = "start\n" + head + gap + "more\n" if keep == "head" else "start\n" + gap + tail
    ingest_json("Get-Content dump.txt", output)
    [entry] = termstore.load_entries()
    for line in KEY_LINES:
        for part in (line[:24], line[-24:]):
            assert part not in entry.output, entry.output


def test_a_complete_private_key_still_collapses_to_one_marker():
    ingest_json("./show-key", "before\n" + PEM + "after\n")
    [entry] = termstore.load_entries()
    assert entry.output.splitlines() == [
        "before",
        "-----BEGIN PRIVATE KEY-----[REDACTED]-----END PRIVATE KEY-----",
        "after",
    ]
