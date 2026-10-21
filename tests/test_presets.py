import json
import os
import subprocess
import sys

import pytest

from codigest.core import presets


@pytest.fixture
def project(tmp_path, monkeypatch):
    root = tmp_path / "proj"
    (root / ".git").mkdir(parents=True)
    monkeypatch.chdir(root)
    monkeypatch.setenv("APPDATA", str(tmp_path / "appdata"))
    monkeypatch.setenv("XDG_CONFIG_HOME", str(tmp_path / "appdata"))
    monkeypatch.setenv("CODIGEST_HOME", str(tmp_path / "home"))
    return root


def write_project(root, text):
    (root / ".codigest").mkdir(exist_ok=True)
    (root / ".codigest" / "config.toml").write_text(text, encoding="utf-8")


def test_options_become_flags_in_sorted_key_order():
    table = {"format": "markdown", "fields": ["command", "output"], "count": 3,
             "failed": True, "here": False, "ratio": 2.0, "tiny": 1e-7}
    assert presets._to_args(table, "t") == [
        "--count", "3", "--failed", "--fields", "command,output", "--format", "markdown",
        "--ratio", "2", "--tiny", "0.0000001",
    ]


def test_a_table_value_has_no_flag_form():
    with pytest.raises(presets.PresetError, match="option nested cannot take table"):
        presets._to_args({"nested": {"a": 1}}, "preset x in f")


def test_the_options_go_right_after_the_command_so_later_ones_win(project):
    write_project(project, '[presets.term.debug]\nformat = "markdown"\ncount = 3\nfailed = true\n')
    assert presets.expand(["term", "--preset", "debug", "-n", "1"]) == [
        "term", "--count", "3", "--failed", "--format", "markdown", "-n", "1",
    ]
    assert presets.expand(["term", "--preset=debug"]) == [
        "term", "--count", "3", "--failed", "--format", "markdown",
    ]
    assert presets.expand(["term", "-n", "2"]) == ["term", "-n", "2"]


def test_a_project_preset_replaces_a_user_preset_of_the_same_name(project):
    user = presets.user_config()
    user.parent.mkdir(parents=True)
    user.write_text('[presets.scan.a]\nmessage = "user"\n[presets.scan.b]\nyes = true\n', encoding="utf-8")
    write_project(project, '[presets.scan.a]\nmessage = "project"\n')
    assert presets.expand(["scan", "--preset", "a"]) == ["scan", "--message", "project"]
    assert presets.expand(["scan", "--preset", "b"]) == ["scan", "--yes"]


def test_the_project_config_is_found_from_a_subfolder_but_not_past_a_git_root(project):
    write_project(project, '[presets.term.p]\nhere = true\n')
    sub = project / "src"
    sub.mkdir()
    os.chdir(sub)
    assert presets.expand(["term", "--preset", "p"]) == ["term", "--here"]
    nested = sub / "inner"
    (nested / ".git").mkdir(parents=True)
    os.chdir(nested)
    with pytest.raises(presets.PresetError, match="no preset"):
        presets.expand(["term", "--preset", "p"])


@pytest.mark.parametrize(("argv", "message"), [
    (["term", "--preset"], "--preset needs a name"),
    (["--preset", "x"], 'no preset "x" for x; define [presets.x.x] in '),
    (["term", "--preset", "missing"], 'no preset "missing" for term; define [presets.term.missing] in '),
])
def test_errors_name_what_to_fix(project, argv, message):
    with pytest.raises(presets.PresetError) as err:
        presets.expand(argv)
    assert message in str(err.value)


def run_cdg(project, tmp_path, *args):
    env = {**os.environ, "APPDATA": str(tmp_path / "appdata"),
           "XDG_CONFIG_HOME": str(tmp_path / "appdata"), "CODIGEST_HOME": str(tmp_path / "home"),
           "NO_COLOR": "1", "COLUMNS": "500", "PYTHONUTF8": "1"}
    return subprocess.run([sys.executable, "-m", "codigest", *args], check=False, cwd=project, env=env,
                          capture_output=True, text=True, encoding="utf-8")


def test_the_command_line_overrides_a_preset_and_an_unknown_one_exits_2(project, tmp_path):
    term = tmp_path / "home" / "term"
    term.mkdir(parents=True)
    lines = [json.dumps({"command": f"echo {i}", "output": str(i), "exit_code": 0}) for i in (1, 2)]
    (term / "history.jsonl").write_text("\n".join(lines) + "\n", encoding="utf-8")
    write_project(project, '[presets.term.both]\nformat = "json"\ncount = 2\nstdout = true\n')

    run = run_cdg(project, tmp_path, "term", "--preset", "both")
    assert len(json.loads(run.stdout)) == 2
    run = run_cdg(project, tmp_path, "term", "--preset=both", "-n", "1")
    assert len(json.loads(run.stdout)) == 1

    run = run_cdg(project, tmp_path, "term", "--preset", "missing")
    assert run.returncode == 2
    assert 'no preset "missing" for term' in run.stderr


def test_a_repeated_option_replaces_the_earlier_one(project, tmp_path):
    (project / "a.py").write_text("x = 1\n")
    run = run_cdg(project, tmp_path, "scan", "-y", "-s", "--format", "json", "--format", "markdown", "-m", "one", "-m", "two")
    assert run.returncode == 0
    assert run.stdout.startswith("# proj\n\ntwo\n")
