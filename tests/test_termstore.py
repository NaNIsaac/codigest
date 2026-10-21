from codigest.core import termstore
from codigest.core.termstore import Entry


def test_crlf_output_survives_normalization():
    assert termstore.normalize_output("a\r\nb\r\n") == "a\nb"


def test_progress_redraw_keeps_last_frame():
    assert termstore.normalize_output("10%\r50%\r100%\ndone") == "100%\ndone"


def test_render_escapes_quotes_in_command_attribute():
    out = termstore.render([Entry(command='python -c "print(1)"', output="1", exit_code=0)])
    assert 'command="python -c &quot;print(1)&quot;"' in out


def test_terminal_context_opt_in_and_escapes_host_and_shell():
    entry = Entry(
        command="Get-Process",
        output="error output",
        exit_code=1,
        cwd="C:/work",
        hostname='LAB-PC & "<node>"',
        shell="pwsh <7>",
    )
    default = termstore.render([entry])
    assert "hostname=" not in default and "shell=" not in default
    with_context = termstore.render([entry], include_context=True)
    assert 'hostname="LAB-PC &amp; &quot;&lt;node&gt;&quot;"' in with_context
    assert 'shell="pwsh &lt;7&gt;"' in with_context
    assert 'command="Get-Process"' in with_context
    assert "<output>\nerror output\n</output>" in with_context


def test_terminal_host_is_recorded_on_append_and_old_entries_remain_compatible(
    tmp_path, monkeypatch
):
    monkeypatch.setenv("CODIGEST_HOME", str(tmp_path))
    monkeypatch.setattr(termstore.socket, "gethostname", lambda: "LAB-PC")
    termstore.append_entry(Entry(command="echo hello", output="hello", shell="powershell"))
    entry = termstore.load_entries()[0]
    assert entry.hostname == "LAB-PC"
    assert 'hostname="LAB-PC"' in termstore.render([entry], include_context=True)
    assert Entry.from_dict({"command": "old record", "output": "ok"}).hostname == ""


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
    assert (
        mcp.handle_message({"jsonrpc": "2.0", "id": 2, "method": "nope"})["error"]["code"] == -32601
    )


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


def test_mcp_native_history_fallback_is_redacted_and_unscoped_only_on_request(
    tmp_path, monkeypatch
):
    from codigest.commands import mcp
    from codigest.core import shellhist

    token = "ghp_" + "a1B2c3D4e5F6g7H8i9J0k1L2"
    hist = tmp_path / "ConsoleHost_history.txt"
    hist.write_text(
        f"cd D:/other-project\ngh auth login --with-token {token}\ndeploy --password=hunter2secret\n",
        encoding="utf-8",
    )
    monkeypatch.setenv("CODIGEST_HOME", str(tmp_path / "home"))
    monkeypatch.setattr(
        shellhist, "_candidates", lambda: [("powershell", hist, shellhist._parse_plain)]
    )

    for args in (
        {"count": 5},
        {"count": 5, "here": True},
        {"count": 5, "here": False, "failed": True},
    ):
        text = mcp._terminal_history(args)
        assert "other-project" not in text and "<command" not in text

    text = mcp._terminal_history({"count": 5, "here": False})
    assert "other-project" in text
    assert token not in text and "hunter2secret" not in text and "[REDACTED]" in text


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


def test_cap_runs_powershell_cmdlets_on_windows():
    import shutil
    import sys

    import pytest

    from codigest.core import runner

    if sys.platform != "win32" or not (shutil.which("pwsh") or shutil.which("powershell")):
        pytest.skip("PowerShell commands exist only on Windows with pwsh or powershell")
    code, output, _ = runner.run(["Write-Output", "a b", "it's"])
    assert code == 0
    assert [line.strip() for line in output.splitlines()] == ["a b", "it's"]
    # A cmdlet, not `ls`: CI runners have Git's ls.exe on PATH, and an executable wins.
    assert runner.run(["Get-ChildItem", "-Name"])[0] == 0
    with pytest.raises(FileNotFoundError):
        runner.run(["codigest-no-such-program-xyz"])
