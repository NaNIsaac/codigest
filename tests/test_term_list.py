"""`cdg term --list` and `-n` ranges."""

import json
import xml.etree.ElementTree as ET
from datetime import datetime

from typer.testing import CliRunner

from codigest.commands import term
from codigest.core import output as output_mod
from codigest.core import termstore


def _seed(tmp_path, monkeypatch, rows):
    monkeypatch.setenv("CODIGEST_HOME", str(tmp_path))
    today = datetime.now().astimezone().replace(hour=9, minute=0, second=0, microsecond=0)
    for i, (command, code, cwd) in enumerate(rows):
        ts = today.replace(minute=i).isoformat(timespec="seconds")
        termstore.append_entry(
            termstore.Entry(
                command=command,
                output=f"out-{command}",
                exit_code=code,
                cwd=cwd,
                started_at=ts,
            )
        )


def test_list_prints_metadata_and_never_copies_even_with_copy_flag(tmp_path, monkeypatch):
    _seed(
        tmp_path,
        monkeypatch,
        [
            ("pytest -q", 1, "/w"),
            ("git status", 0, "/w"),
        ],
    )
    copied = []
    monkeypatch.setattr(output_mod.pyperclip, "copy", lambda text: copied.append(text))

    result = CliRunner().invoke(term.app, ["--list", "--copy"])

    assert result.exit_code == 0, result.output
    assert result.stdout.splitlines() == [
        "2. 09:00  exit 1  /w  pytest -q",
        "1. 09:01  exit 0  /w  git status",
    ]
    assert "out-pytest" not in result.stdout and "out-git" not in result.stdout
    assert "Copied" not in result.stdout
    assert copied == []


def test_list_index_matches_what_dash_i_picks_under_filters(tmp_path, monkeypatch):
    _seed(
        tmp_path,
        monkeypatch,
        [
            ("build", 1, "/w"),
            ("test", 0, "/w"),
            ("lint", 2, "/w"),
            ("deploy", 3, "/w"),
        ],
    )
    runner = CliRunner()

    listed = runner.invoke(term.app, ["-f", "-n", "2-3", "--list"])
    assert listed.exit_code == 0, listed.output
    assert listed.stdout.splitlines() == [
        "3. 09:00  exit 1  /w  build",
        "2. 09:02  exit 2  /w  lint",
    ]

    single = runner.invoke(term.app, ["-f", "--index", "2", "--list"])
    assert single.exit_code == 0, single.output
    assert single.stdout.strip() == "2. 09:02  exit 2  /w  lint"

    picked = runner.invoke(term.app, ["-f", "--index", "2", "--stdout", "--no-copy"])
    assert picked.exit_code == 0, picked.output
    assert 'command="lint"' in picked.stdout


def test_count_range_forward_and_reversed_picks_commands_oldest_first(tmp_path, monkeypatch):
    _seed(
        tmp_path,
        monkeypatch,
        [(f"cmd{n}", 0, "/w") for n in range(1, 7)],
    )
    runner = CliRunner()

    fwd = runner.invoke(term.app, ["-n", "3-5", "--stdout", "--no-copy"])
    rev = runner.invoke(term.app, ["-n", "5-3", "--stdout", "--no-copy"])
    assert fwd.exit_code == 0, fwd.output
    assert rev.exit_code == 0, rev.output
    assert fwd.stdout == rev.stdout

    root = ET.fromstring(fwd.stdout)
    cmds = [(el.attrib["index"], el.attrib["command"], el.findtext("output")) for el in root]
    assert cmds == [
        ("5", "cmd2", "\nout-cmd2\n"),
        ("4", "cmd3", "\nout-cmd3\n"),
        ("3", "cmd4", "\nout-cmd4\n"),
    ]


def test_count_bad_and_unreachable_ranges_fail_clearly(tmp_path, monkeypatch):
    _seed(tmp_path, monkeypatch, [("cmd1", 0, "/w"), ("cmd2", 0, "/w")])
    runner = CliRunner()

    for bad in ("0", "0-2", "3-", "abc", "1-2-3"):
        res = runner.invoke(term.app, ["-n", bad])
        assert res.exit_code != 0
        assert "Error" in res.output or "Invalid" in res.output

    neg = runner.invoke(term.app, ["-n=-2"])
    assert neg.exit_code != 0

    oob = runner.invoke(term.app, ["-n", "3-5"])
    assert oob.exit_code != 0
    assert "Only 2 entries available" in oob.output


def test_list_format_json_outputs_rows_and_supports_ranges(tmp_path, monkeypatch):
    _seed(
        tmp_path,
        monkeypatch,
        [
            ("cmd1", 0, "/a"),
            ("cmd2", 1, "/b"),
            ("cmd3", 0, "/c"),
            ("cmd4", 2, "/d"),
        ],
    )
    copied = []
    monkeypatch.setattr(output_mod.pyperclip, "copy", lambda text: copied.append(text))

    res = CliRunner().invoke(term.app, ["--list", "--format", "json", "-n", "2-3"])

    assert res.exit_code == 0, res.output
    rows = json.loads(res.stdout)
    assert rows == [
        {"index": 3, "time": "09:01", "exit_code": 1, "cwd": "/b", "command": "cmd2"},
        {"index": 2, "time": "09:02", "exit_code": 0, "cwd": "/c", "command": "cmd3"},
    ]
    assert copied == []
