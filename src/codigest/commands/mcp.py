"""
`cdg mcp` -- expose Codigest to AI agents as an MCP server (stdio).

Register once, e.g. in Claude Code:

    claude mcp add codigest -- cdg mcp

Hand-rolled JSON-RPC over newline-delimited stdio: the protocol surface used
here (initialize, tools/list, tools/call, ping) is small enough that an SDK
dependency would only add install weight.

Project tools shell out to the regular CLI (`-s --no-copy`), so the agent sees
exactly what a human would paste, and stdout stays clean for the protocol.
"""
import json
import subprocess
import sys
from pathlib import Path
from importlib.metadata import PackageNotFoundError, version
from typing import Any

lazy from ..core import termstore, shellhist

type JSON = dict[str, Any]

PROTOCOL_VERSION = "2025-06-18"

_PATH_PROP = {"path": {"type": "string", "description": "Project directory (default: server cwd)"}}
_MESSAGE_PROP = {"message": {"type": "string", "description": "Instruction to embed in the output"}}

TOOLS: list[JSON] = [
    {
        "name": "terminal_history",
        "description": (
            "Recent terminal commands with their full output, exit code and cwd, as captured "
            "by Codigest. Use this to see what the user just ran and why it failed, instead of "
            "asking them to paste it."
        ),
        "inputSchema": {
            "type": "object",
            "properties": {
                "count": {"type": "integer", "minimum": 1, "default": 1, "description": "How many recent commands"},
                "failed": {"type": "boolean", "default": False, "description": "Only commands that exited non-zero"},
                "here": {
                    "type": "boolean",
                    "default": True,
                    "description": "Only commands run in the server's working directory (the project) or below",
                },
            },
        },
    },
    {
        "name": "scan",
        "description": "Full project snapshot: tree plus file contents as XML, filtered by the project's Codigest config.",
        "inputSchema": {
            "type": "object",
            "properties": {
                **_PATH_PROP,
                **_MESSAGE_PROP,
                "targets": {"type": "array", "items": {"type": "string"}, "description": "Limit to these files/dirs"},
                "resolve": {"type": "boolean", "default": False, "description": "Follow imports of the targets"},
            },
        },
    },
    {
        "name": "diff",
        "description": "Code changes since the last Codigest anchor (requires `cdg init` in the project).",
        "inputSchema": {"type": "object", "properties": {**_PATH_PROP, **_MESSAGE_PROP}},
    },
    {
        "name": "semdiff",
        "description": "Structural (function/class level) changes since the last Codigest anchor.",
        "inputSchema": {"type": "object", "properties": {**_PATH_PROP, **_MESSAGE_PROP}},
    },
    {
        "name": "digest",
        "description": "Architecture digest: signatures and docstrings without bodies.",
        "inputSchema": {"type": "object", "properties": {**_PATH_PROP, **_MESSAGE_PROP}},
    },
    {
        "name": "tree",
        "description": "Project directory tree.",
        "inputSchema": {"type": "object", "properties": {**_PATH_PROP}},
    },
]


def _terminal_history(args: JSON) -> str:
    count = int(args.get("count", 1))
    if not (entries := termstore.load_entries()):
        # Nothing captured at all: native shell history still names the commands.
        entries, _ = shellhist.read_recent(count)
        return termstore.render(entries)
    under = Path.cwd() if args.get("here", True) else None
    return termstore.render(termstore.select(entries, failed=bool(args.get("failed")), under=under)[-count:])


def _cli(command: str, args: JSON) -> str:
    argv = [sys.executable, "-m", "codigest", command, "-s", "--no-copy"]
    if command == "scan":
        argv.append("-y")
    if args.get("resolve"):
        argv.append("-r")
    if message := args.get("message"):
        argv += ["-m", message]
    match command:
        case "scan":
            argv += [str(t) for t in args.get("targets") or []] or ([args["path"]] if "path" in args else [])
        case _ if "path" in args:
            argv.append(args["path"])

    result = subprocess.run(argv, capture_output=True, text=True, encoding="utf-8", errors="replace")
    if result.returncode != 0:
        raise RuntimeError(result.stderr.strip() or result.stdout.strip() or f"exit {result.returncode}")
    return result.stdout


def _call_tool(name: str, args: JSON) -> JSON:
    try:
        match name:
            case "terminal_history":
                text = _terminal_history(args)
            case "scan" | "diff" | "semdiff" | "digest" | "tree":
                text = _cli(name, args)
            case _:
                raise ValueError(f"Unknown tool: {name}")
    except Exception as e:  # reported to the agent as a tool error, not a protocol error
        return {"content": [{"type": "text", "text": str(e)}], "isError": True}
    return {"content": [{"type": "text", "text": text}], "isError": False}


def _server_version() -> str:
    try:
        return version("codigest")
    except PackageNotFoundError:
        return "dev"


def handle_message(msg: JSON) -> JSON | None:
    """Returns the JSON-RPC response for a request, or None for notifications."""
    msg_id = msg.get("id")
    params = msg.get("params") or {}

    match msg.get("method"):
        case "initialize":
            result: JSON = {
                "protocolVersion": params.get("protocolVersion", PROTOCOL_VERSION),
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "codigest", "version": _server_version()},
            }
        case "tools/list":
            result = {"tools": TOOLS}
        case "tools/call":
            result = _call_tool(params.get("name", ""), params.get("arguments") or {})
        case "ping":
            result = {}
        case _ if msg_id is None:
            return None  # notifications (initialized, cancelled, ...) need no reply
        case method:
            return {"jsonrpc": "2.0", "id": msg_id, "error": {"code": -32601, "message": f"Method not found: {method}"}}

    return None if msg_id is None else {"jsonrpc": "2.0", "id": msg_id, "result": result}


def handle() -> None:
    """[Agents] Serve Codigest tools over MCP (stdio). Register: claude mcp add codigest -- cdg mcp"""
    stdin = open(sys.stdin.fileno(), encoding="utf-8", closefd=False)
    stdout = open(sys.stdout.fileno(), "w", encoding="utf-8", newline="\n", closefd=False)
    for line in stdin:
        if not line.strip():
            continue
        try:
            msg = json.loads(line)
        except json.JSONDecodeError:
            response: JSON | None = {"jsonrpc": "2.0", "id": None, "error": {"code": -32700, "message": "Parse error"}}
        else:
            response = handle_message(msg)
        if response is not None:
            stdout.write(json.dumps(response, ensure_ascii=False) + "\n")
            stdout.flush()
