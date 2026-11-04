# MCP Integration

`cdg mcp` is an MCP server that lets a coding agent request the same context you would paste. It is listed in the MCP registry as `io.github.NaNIsaac/codigest`.

## Register it

Claude Code:

```bash
claude mcp add --scope user codigest -- cdg mcp
```

Other clients (Cursor, VS Code, Claude Desktop): add a stdio server with command `cdg` and args `["mcp"]`. Most clients take a configuration of this shape:

```json
{
  "mcpServers": {
    "codigest": {
      "command": "cdg",
      "args": ["mcp"]
    }
  }
}
```

The server works on the project it was started in: its working directory is the project. Clients that start servers in the project folder need nothing more. If yours starts them elsewhere, set the working directory in the client's server settings.

## Tools

| Tool | Returns | Arguments |
| --- | --- | --- |
| `pack` | Last failure, the files it names, changes and their imports, cut to a budget | `targets`, `with`, `budget`, `message` |
| `terminal_history` | Recent commands with output and exit code | `count`, `failed`, `here` |
| `scan` | Project snapshot | `path`, `targets`, `resolve`, `message` |
| `diff` | Text changes since the baseline | `path`, `message` |
| `semdiff` | Function and class level changes since the baseline | `path`, `message` |
| `digest` | Signatures and docstrings only | `path`, `message` |
| `tree` | Directory tree | `path` |

The tools return the same text the CLI prints. Each one runs the regular command with `-s --no-copy`, so an agent sees exactly what you would paste, and the clipboard is never used. See [Output Formats](/output-formats).

### Arguments

- `targets`: a list of files or folders. For `pack`, those are included first, and naming files turns the automatic sources off, as on the command line.
- `with`: for `pack`, a list of any of `failure`, `changes`, `tests`, `imports`. Default: all of them, or none when `targets` are given.
- `budget`: for `pack`, an estimated token budget. Default 30000, minimum 1000.
- `message`: an instruction embedded in the output.
- `resolve`: for `scan`, follow imports of the targets.
- `path`: a project directory, relative to the server's working directory.
- `count`: for `terminal_history`, how many recent commands. Default 1.
- `failed`: for `terminal_history`, only commands that exited non-zero. Default false.
- `here`: for `terminal_history`, only commands run in the project or below. Default true.

`scan` through MCP never stops to ask about size.

## Limits

- None of the tools moves the baseline. Only you set it with `cdg mark`.
- Paths outside the project the server was started in are refused, including through symlinks and junctions. The tool returns an error instead.
- Errors come back as tool results marked as errors, not as protocol errors, so the agent can read the reason.
- `terminal_history` needs captures, made with `cdg cap` or the shell hook. With none at all it returns a message saying so, unless asked with `here=false` and `failed=false`, in which case it falls back to the shell history: commands only, with secrets masked.

## The protocol

The server is a small JSON-RPC loop over stdio, one message per line. It answers `initialize`, `tools/list`, `tools/call` and `ping`, and ignores notifications. There is no MCP SDK dependency.

Listing the tools by hand:

```bash
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"t","version":"0"}}}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' | cdg mcp
```

The second response lists, in this order: `terminal_history`, `pack`, `scan`, `diff`, `semdiff`, `digest`, `tree`.


## GitHub repositories (Python build)

Start the Python server with explicit remote access to expose `scan_github`:

```sh
uvx --python 3.15 codigest mcp --allow-remote
```

This is unreleased source functionality until a release containing this change is published.
The default `cdg mcp` tool list and local project boundary are unchanged. The Rust build does
not yet implement this option. A GitHub scan never executes the repository's code.

`scan_github` accepts `repository` (OWNER/REPO or a github.com URL), optional `ref`,
`paths` (a list, including FILE:START-END ranges), and `format` (xml, markdown, json).
It returns the CLI snapshot with the resolved commit and omitted files. Remote digest, tree,
diff and pack are not provided. Each remote child process has a 110-second timeout.

Public scans do not use GitHub credentials. To let the server use the operator's existing
GitHub credentials for private repositories, start with both `--allow-remote --auth`.
A tool caller cannot enable authentication or change that startup policy.

For ChatGPT cloud, connect this stdio command through Secure MCP Tunnel; starting the command
alone does not register a ChatGPT plugin. See [private ChatGPT setup](chatgpt-mcp).
