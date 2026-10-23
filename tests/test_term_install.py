from pathlib import Path

from typer.testing import CliRunner

from codigest.commands import term
from codigest.core import termstore


def test_ingest_options_and_output_file(tmp_path, monkeypatch):
    monkeypatch.setenv("CODIGEST_HOME", str(tmp_path / "home"))
    out = tmp_path / "out"
    out.write_bytes("echo 'a' \"b\" $HOME *.py | y\0".encode() + 'say "hi" — ünïcode\n'.encode())
    result = CliRunner().invoke(
        term.app,
        [
            "ingest",
            f"--file={out}",
            "--exit-code=3",
            f"--cwd={tmp_path}",
            "--shell=bash",
            "--start=1700000000,25",
            "--end=1700000001.5",
        ],
    )
    assert result.exit_code == 0, result.output
    [entry] = termstore.load_entries()
    assert entry.command == "echo 'a' \"b\" $HOME *.py | y"
    assert entry.output == 'say "hi" — ünïcode'
    assert (entry.exit_code, entry.shell, entry.duration_ms) == (3, "bash", 1250)
    assert (
        entry.cwd == str(tmp_path) and entry.started_at[:2] == "20" and entry.started_at[-6] in "+-"
    )
    assert not out.exists()


def test_ingest_stdin_json_still_works(tmp_path, monkeypatch):
    monkeypatch.setenv("CODIGEST_HOME", str(tmp_path))
    result = CliRunner().invoke(
        term.app, ["ingest"], input='{"command": "ls", "output": "x", "exit_code": 0}'
    )
    assert result.exit_code == 0
    assert [e.command for e in termstore.load_entries()] == ["ls"]


def test_install_uninstall_rc_files(tmp_path, monkeypatch):
    monkeypatch.setattr(term.Path, "home", lambda: tmp_path)
    monkeypatch.setenv("CODIGEST_HOME", str(tmp_path / "data"))
    monkeypatch.delenv("ZDOTDIR", raising=False)
    monkeypatch.setattr(term, "_powershell_profiles", lambda: [])
    monkeypatch.setattr(term.shutil, "which", lambda name: "/bin/bash" if name == "bash" else None)
    bashrc, zshrc = tmp_path / ".bashrc", tmp_path / ".zshrc"
    bashrc.write_bytes(b"alias g=git\n")
    runner = CliRunner()

    assert runner.invoke(term.app, ["install"]).exit_code == 0  # auto: bash only
    assert runner.invoke(term.app, ["install"]).exit_code == 0  # idempotent
    text = bashrc.read_bytes().decode()
    assert text.startswith("alias g=git\n\n" + term._BEGIN) and text.count(term._BEGIN) == 1
    # The profile loads a copy in the data folder, not the package's own file.
    copy = tmp_path / "data" / "shell" / "hook.sh"
    assert copy.read_bytes() == term.SH_HOOK_PATH.read_bytes()
    assert f". '{term._posix(copy)}'" in text and "\r" not in text
    assert not text.startswith("\ufeff") and not zshrc.exists()

    assert runner.invoke(term.app, ["install", "--shell", "zsh"]).exit_code == 0
    assert term._BEGIN in zshrc.read_text()
    assert runner.invoke(term.app, ["install", "--shell", "fish"]).exit_code != 0

    assert runner.invoke(term.app, ["uninstall"]).exit_code == 0  # auto removes everywhere
    assert bashrc.read_bytes() == b"alias g=git\n"
    assert not zshrc.exists()  # install created it


def test_posix_hook_path():
    assert term._posix(Path("C:/Users/me/hook.sh")).endswith("/Users/me/hook.sh")
    if Path("C:/x").drive:
        assert term._posix(Path(r"C:\Users\me\hook.sh")) == "/c/Users/me/hook.sh"
