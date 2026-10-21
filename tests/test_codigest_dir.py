"""`.codigest/` must not show up in `git status` of a project that never ignored it."""

import subprocess

from typer.testing import CliRunner

from codigest.main import app


def git_status(root):
    return subprocess.run(
        ["git", "status", "--short"], cwd=root, text=True, capture_output=True, check=True
    ).stdout


def project(tmp_path, monkeypatch):
    subprocess.run(["git", "init", "-q"], cwd=tmp_path, check=True)
    (tmp_path / "a.py").write_text("x = 1\n", encoding="utf-8")
    monkeypatch.chdir(tmp_path)


def test_mark_keeps_the_folder_out_of_git_status(tmp_path, monkeypatch):
    project(tmp_path, monkeypatch)

    CliRunner().invoke(app, ["mark"])

    assert (tmp_path / ".codigest" / ".gitignore").read_text(encoding="utf-8") == "*\n"
    assert ".codigest" not in git_status(tmp_path)
    assert "a.py" in git_status(tmp_path)


def test_init_does_the_same_and_still_edits_the_project_gitignore(tmp_path, monkeypatch):
    project(tmp_path, monkeypatch)
    (tmp_path / ".gitignore").write_text("build/\n", encoding="utf-8")

    result = CliRunner().invoke(app, ["init"])

    assert result.exit_code == 0
    assert (tmp_path / ".codigest" / "config.toml").is_file()
    assert (tmp_path / ".codigest" / ".gitignore").read_text(encoding="utf-8") == "*\n"
    assert (tmp_path / ".gitignore").read_text(encoding="utf-8").splitlines()[-1] == ".codigest/"
    assert ".codigest" not in git_status(tmp_path)


def test_an_existing_ignore_file_is_left_alone(tmp_path, monkeypatch):
    project(tmp_path, monkeypatch)
    (tmp_path / ".codigest").mkdir()
    (tmp_path / ".codigest" / ".gitignore").write_text("anchor/\n", encoding="utf-8")

    CliRunner().invoke(app, ["mark"])

    assert (tmp_path / ".codigest" / ".gitignore").read_text(encoding="utf-8") == "anchor/\n"
