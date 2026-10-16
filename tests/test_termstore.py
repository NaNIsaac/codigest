from codigest.core import termstore
from codigest.core.termstore import Entry


def test_crlf_output_survives_normalization():
    assert termstore.normalize_output("a\r\nb\r\n") == "a\nb"


def test_progress_redraw_keeps_last_frame():
    assert termstore.normalize_output("10%\r50%\r100%\ndone") == "100%\ndone"


def test_render_escapes_quotes_in_command_attribute():
    out = termstore.render([Entry(command='python -c "print(1)"', output="1", exit_code=0)])
    assert 'command="python -c &quot;print(1)&quot;"' in out



def test_profile_block_strip_keeps_user_content():
    from codigest.commands import term
    block = f"{term._BEGIN}\nhook\n{term._END}\n"
    assert term._strip_block("Set-Alias g git\n\n" + block) == "Set-Alias g git\n"
    assert term._strip_block(block) == ""



def test_mcp_lists_tools_and_ignores_notifications():
    from codigest.commands import mcp
    listed = mcp.handle_message({"jsonrpc": "2.0", "id": 1, "method": "tools/list"})
    assert "terminal_history" in [t["name"] for t in listed["result"]["tools"]]
    assert mcp.handle_message({"jsonrpc": "2.0", "method": "notifications/initialized"}) is None
    assert mcp.handle_message({"jsonrpc": "2.0", "id": 2, "method": "nope"})["error"]["code"] == -32601



def test_select_filters_failed_and_directory(tmp_path):
    project, other = tmp_path / "proj", tmp_path / "proj-other"
    entries = [
        Entry(command="a", exit_code=1, cwd=str(project / "sub")),
        Entry(command="b", exit_code=0, cwd=str(project)),
        Entry(command="c", exit_code=2, cwd=str(other)),  # shares the prefix, not the directory
        Entry(command="d", exit_code=None, cwd=""),
    ]
    assert [e.command for e in termstore.select(entries, under=project)] == ["a", "b"]
    assert [e.command for e in termstore.select(entries, failed=True)] == ["a", "c"]
    assert [e.command for e in termstore.select(entries, failed=True, under=project)] == ["a"]


if __name__ == "__main__":
    test_crlf_output_survives_normalization()
    test_progress_redraw_keeps_last_frame()
    test_render_escapes_quotes_in_command_attribute()
    test_profile_block_strip_keeps_user_content()
    test_mcp_lists_tools_and_ignores_notifications()
    print("ok")


def test_mcp_refuses_paths_outside_the_project(tmp_path, monkeypatch):
    from codigest.commands import mcp
    (tmp_path / "proj").mkdir()
    monkeypatch.chdir(tmp_path / "proj")
    for path in ("..", str(tmp_path), "../proj/../x"):
        result = mcp._call_tool("tree", {"path": path})
        assert result["isError"] and "outside the project" in result["content"][0]["text"]
    assert mcp._in_project(".") == str((tmp_path / "proj").resolve())


def test_cap_resolves_cmd_shims_and_relative_paths_on_windows(tmp_path, monkeypatch):
    import sys

    import pytest

    from codigest.core import runner
    if sys.platform != "win32":
        pytest.skip("CreateProcess lookup rules are Windows-only")
    (tmp_path / "tool.cmd").write_text("@echo off\r\nexit /b 3\r\n")
    monkeypatch.setenv("PATH", str(tmp_path))
    assert runner.build_argv(["tool", "-x"])[0].lower() == str(tmp_path / "tool.cmd").lower()
    assert runner.run(["tool"])[0] == 3
