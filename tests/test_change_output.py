"""`diff`, `semdiff` and `mark` on a pipe: the payload on stdout, the banners on stderr."""
import subprocess

from typer.testing import CliRunner

from codigest.main import app


def project(tmp_path, monkeypatch, text="x = 1\n"):
    (tmp_path / ".git").mkdir()
    (tmp_path / "a.py").write_text(text, encoding="utf-8")
    monkeypatch.chdir(tmp_path)
    return tmp_path / "a.py"


def test_mark_reports_on_stderr(tmp_path, monkeypatch):
    project(tmp_path, monkeypatch)

    result = CliRunner().invoke(app, ["mark"])

    assert "Baseline set" in result.stderr
    assert result.stdout == ""


def test_diff_stdout_is_only_the_diff(tmp_path, monkeypatch):
    a = project(tmp_path, monkeypatch)
    CliRunner().invoke(app, ["mark"])
    a.write_text("x = 2\n", encoding="utf-8")

    result = CliRunner().invoke(app, ["diff"])

    assert result.stdout.startswith("<changes")
    assert "Changes Detected" not in result.stdout
    assert "Changes Detected" in result.stderr


def test_semdiff_stdout_is_only_the_report(tmp_path, monkeypatch):
    a = project(tmp_path, monkeypatch, "def f():\n    return 1\n")
    CliRunner().invoke(app, ["mark"])
    a.write_text("def f():\n    return 2\n", encoding="utf-8")

    result = CliRunner().invoke(app, ["semdiff"])

    assert result.stdout.startswith("<semantic_diff")
    assert "SemDiff Generated" not in result.stdout
    assert "SemDiff Generated" in result.stderr


def test_diff_headers_have_a_and_b_prefixes_and_apply_with_git(tmp_path, monkeypatch):
    a = project(tmp_path, monkeypatch)
    before = a.read_text(encoding="utf-8")
    CliRunner().invoke(app, ["mark"])
    a.write_text("x = 2\ny = 3\n", encoding="utf-8")

    out = CliRunner().invoke(app, ["diff"]).stdout

    lines = out.splitlines()
    assert "diff --git a/a.py b/a.py" in lines
    assert "--- a/a.py" in lines and "+++ b/a.py" in lines
    patch = "\n".join(lines[1:-1]) + "\n"  # between the <changes> tags
    old = tmp_path / "old"
    old.mkdir()
    (old / "a.py").write_text(before, encoding="utf-8")
    applied = subprocess.run(
        ["git", "apply", "--check", "-"], input=patch, cwd=old, text=True, capture_output=True, check=False
    )
    assert applied.returncode == 0, applied.stderr
