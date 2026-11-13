"""Remote MCP scans preserve the CLI's path/credential boundary."""

import json
import subprocess

import pytest

from codigest.commands import mcp


def test_remote_tool_is_opt_in():
    msg = {"id": 1, "method": "tools/list"}
    assert mcp.handle_message(msg)["result"]["tools"] == mcp.TOOLS
    enabled = mcp.handle_message(msg, allow_remote=True)["result"]["tools"]
    assert enabled[-1]["name"] == "scan_github"
    result = mcp._call_tool("scan_github", {"repository": "owner/repo"})
    assert result["isError"] and "--allow-remote" in result["content"][0]["text"]


@pytest.mark.parametrize("auth", [False, True])
def test_remote_arguments_and_auth_are_controlled_by_operator(monkeypatch, auth):
    calls = []

    def run(argv, **kwargs):
        calls.append((argv, kwargs))
        return subprocess.CompletedProcess(argv, 0, "snapshot", "")

    monkeypatch.setattr(mcp.subprocess, "run", run)
    result = mcp._call_tool(
        "scan_github",
        {
            "repository": "owner/repo",
            "ref": "main",
            "paths": ["src/app.py:1-3"],
            "format": "json",
        },
        allow_remote=True,
        auth=auth,
    )
    assert not result["isError"]
    argv, kwargs = calls[0]
    assert ("--auth" in argv) == auth
    assert "--path=src/app.py:1-3" in argv
    assert "--ref=main" in argv and "--format=json" in argv
    assert kwargs["timeout"] == 110 and "shell" not in kwargs


@pytest.mark.parametrize(
    "args",
    [
        {"repository": "https://evil.example/repo"},
        {"repository": "owner/repo", "paths": ["../outside"]},
        {"repository": "owner/repo", "paths": "src"},
        {"repository": "owner/repo", "format": "shell"},
        {"repository": "owner/repo", "auth": True},
    ],
)
def test_bad_input_never_starts_a_process(monkeypatch, args):
    def forbidden(*a, **kw):
        pytest.fail("invalid input started a child")

    monkeypatch.setattr(mcp.subprocess, "run", forbidden)
    assert mcp._call_tool("scan_github", args, allow_remote=True)["isError"]


def test_local_path_boundary_unchanged(tmp_path, monkeypatch):
    monkeypatch.chdir(tmp_path)
    result = mcp._call_tool("scan", {"targets": ["../outside"]}, allow_remote=True)
    assert result["isError"] and "outside the project" in result["content"][0]["text"]


def test_remote_protocol_through_cli(tmp_path):
    messages = (
        "\n".join(
            json.dumps(m)
            for m in [
                {"jsonrpc": "2.0", "id": 1, "method": "tools/list"},
                {
                    "jsonrpc": "2.0",
                    "id": 2,
                    "method": "tools/call",
                    "params": {
                        "name": "scan_github",
                        "arguments": {"repository": "https://evil.example/x"},
                    },
                },
            ]
        )
        + "\n"
    )
    run = subprocess.run(
        [mcp.sys.executable, "-m", "codigest", "mcp", "--allow-remote"],
        input=messages,
        text=True,
        capture_output=True,
        cwd=tmp_path,
        timeout=20,
        check=False,
    )
    assert run.returncode == 0, run.stderr
    responses = [json.loads(line) for line in run.stdout.splitlines()]
    assert responses[0]["result"]["tools"][-1]["name"] == "scan_github"
    assert responses[1]["result"]["isError"]


def test_remote_scan_over_real_stdio(tmp_path, monkeypatch):
    from fake_github import FakeGitHub

    repo = tmp_path / "repo"
    repo.mkdir()
    (repo / "hello.py").write_text("answer = 42\n")
    work = tmp_path / "work"
    work.mkdir()
    with FakeGitHub(repo).running() as fake:
        monkeypatch.setenv("CODIGEST_GITHUB_API", fake.api)
        monkeypatch.setenv("CODIGEST_GITHUB_RAW", fake.raw)
        request = {
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {"name": "scan_github", "arguments": {"repository": "octo/sample"}},
        }
        result = subprocess.run(
            [mcp.sys.executable, "-m", "codigest", "mcp", "--allow-remote"],
            input=json.dumps(request) + "\n",
            text=True,
            capture_output=True,
            cwd=work,
            timeout=20,
            check=False,
        )
    assert result.returncode == 0, result.stderr
    response = json.loads(result.stdout)["result"]
    assert not response["isError"], response
    assert "answer = 42" in response["content"][0]["text"]
    assert list(work.iterdir()) == []
