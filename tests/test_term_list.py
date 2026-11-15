"""`cdg term --list` prints what was captured without the output and never copies."""

from typer.testing import CliRunner

from codigest.commands import term
from codigest.core import termstore


def test_list_prints_indexed_commands_without_their_output(tmp_path, monkeypatch):
    monkeypatch.setenv("CODIGEST_HOME", str(tmp_path))
    termstore.append_entry(
        termstore.Entry(command="pytest -q", output="FAILED details", exit_code=1)
    )
    termstore.append_entry(termstore.Entry(command="git status", output="clean", exit_code=0))

    result = CliRunner().invoke(term.app, ["--list"])

    assert result.exit_code == 0, result.output
    assert result.stdout.splitlines() == ["2. pytest -q (exit 1)", "1. git status"]
    assert "FAILED details" not in result.stdout and "clean" not in result.stdout
    assert "Copied" not in result.stdout


def test_list_respects_failed_and_index(tmp_path, monkeypatch):
    monkeypatch.setenv("CODIGEST_HOME", str(tmp_path))
    for command, code in (("build", 1), ("test", 0), ("deploy", 2)):
        termstore.append_entry(termstore.Entry(command=command, output="x", exit_code=code))

    result = CliRunner().invoke(term.app, ["-f", "--index", "2", "--list"])

    assert result.exit_code == 0, result.output
    assert result.stdout.strip() == "1. build (exit 1)"
