import json

from typer.testing import CliRunner

from codigest.core import termstore
from codigest.core.formats import fence, fenced, language
from codigest.core.termstore import Entry
from codigest.main import app


def test_the_fence_outgrows_backticks_in_the_content():
    assert fence("plain") == "```"
    assert fence("a ```` b") == "`````"
    assert fenced("x = 1\n", "a.py") == "```python\nx = 1\n```"


def test_language_comes_from_the_extension():
    assert language("src/App.PY") == "python"
    assert language("Makefile") == ""
    assert language("a.unknown") == ""


def history() -> list[Entry]:
    return [
        Entry(
            command="pytest -q",
            output="1 failed\nuse ``` here",
            exit_code=1,
            cwd="/w",
            started_at="2026-01-01T10:00:00+09:00",
            duration_ms=1500,
            hostname="PC",
        ),
        Entry(
            command="echo hi",
            output="hi",
            exit_code=0,
            cwd="/w",
            started_at="2026-01-01T10:00:05+09:00",
        ),
    ]


def test_term_markdown_puts_each_command_and_its_output_in_one_console_block():
    assert termstore.render_markdown(history()) == (
        "## Command 2 (exit 1)\n\n- cwd: /w\n- duration: 1.5s\n- at: 2026-01-01T10:00:00+09:00\n\n"
        "````console\n$ pytest -q\n1 failed\nuse ``` here\n````\n\n"
        "## Command 1 (exit 0)\n\n- cwd: /w\n- at: 2026-01-01T10:00:05+09:00\n\n"
        "```console\n$ echo hi\nhi\n```"
    )


def test_term_markdown_notes_what_was_not_captured():
    entry = Entry(command="ls", captured=False, truncated=True, note="from history")
    text = termstore.render_markdown([entry])
    assert text == (
        "## Command 1\n\n- truncated: middle of the output left out\n"
        "- output: not stored (read from shell history)\n- note: from history\n\n"
        "```console\n$ ls\n```"
    )


def test_term_json_is_an_array_indexed_from_the_latest():
    items = json.loads(termstore.render_json(history()))
    assert [i["index"] for i in items] == [2, 1]
    assert list(items[0]) == [
        "index",
        "command",
        "exit_code",
        "cwd",
        "started_at",
        "duration_ms",
        "output",
        "output_available",
        "truncated",
    ]
    assert "hostname" not in items[0]
    with_context = json.loads(termstore.render_json(history(), include_context=True))
    assert list(with_context[0])[-2:] == ["hostname", "shell"]


def test_term_json_keeps_non_ascii_and_indents_by_two():
    text = termstore.render_json([Entry(command="echo é", output="é")])
    assert '\n    "command": "echo é"' in text


def test_scan_formats_leave_the_xml_cache_out(tmp_path, monkeypatch):
    (tmp_path / ".git").mkdir()
    (tmp_path / "a.py").write_text("x = 1\ny = 2\n")
    (tmp_path / "b.md").write_text("```\nq\n```\n")
    monkeypatch.chdir(tmp_path)
    runner = CliRunner()

    md = runner.invoke(
        app, ["scan", "a.py:2", "b.md", "--format", "markdown", "-m", "fix it", "-s", "--no-copy"]
    )
    assert md.exit_code == 0
    assert md.stdout.startswith(f"# {tmp_path.name}\n\nfix it\n\n## Structure\n\n```\n")
    assert "## a.py (lines 2)\n\n```python\n2: y = 2\n```\n\n" in md.stdout
    assert "## b.md\n\n````markdown\n```\nq\n```\n````\n" in md.stdout

    js = runner.invoke(app, ["scan", "a.py:1-2", "b.md", "--format", "json", "-s", "--no-copy"])
    doc = json.loads(js.stdout)
    assert list(doc) == ["project", "instruction", "structure", "files"]
    assert doc["files"][0] == {"path": "a.py", "lines": "1-2", "content": "1: x = 1\n2: y = 2"}
    assert doc["files"][1] == {"path": "b.md", "content": "```\nq\n```\n"}
    assert not (tmp_path / ".codigest").exists()
