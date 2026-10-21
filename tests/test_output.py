from typer.testing import CliRunner

from codigest.main import app


def test_reading_commands_leave_nothing_in_the_project_and_o_writes_a_file(tmp_path, monkeypatch):
    (tmp_path / ".git").mkdir()
    (tmp_path / "a.py").write_text("def f():\n    return 1\n")
    monkeypatch.chdir(tmp_path)
    runner = CliRunner()
    for cmd in (["scan", "-y"], ["digest"], ["tree"]):
        assert runner.invoke(app, [*cmd, "--no-copy"]).exit_code == 0
    assert not (tmp_path / ".codigest").exists()

    runner.invoke(app, ["scan", "-y", "--no-copy", "-o", "ctx.xml"])
    assert (
        (tmp_path / "ctx.xml").read_text(encoding="utf-8").startswith('<project_snapshot project="')
    )


def test_init_writes_the_config_but_sets_no_baseline(tmp_path, monkeypatch):
    monkeypatch.chdir(tmp_path)
    CliRunner().invoke(app, ["init"])
    assert (tmp_path / ".codigest" / "config.toml").is_file()
    assert not (tmp_path / ".codigest" / "anchor").exists()
