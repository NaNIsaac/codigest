"""`cdg status` on a pipe, where rich assumes 80 columns."""

from typer.testing import CliRunner

from codigest.main import app


def test_a_long_project_path_stays_on_one_line(tmp_path, monkeypatch):
    root = tmp_path / ("deeply-nested-directory-" * 5) / "project"
    (root / ".git").mkdir(parents=True)
    (root / "a.py").write_text("x = 1\n", encoding="utf-8")
    monkeypatch.chdir(root)

    out = CliRunner().invoke(app, ["status"]).stdout

    assert len(str(root)) > 80
    first = out.splitlines()[0]
    assert first.startswith("project") and first.endswith(str(root))
