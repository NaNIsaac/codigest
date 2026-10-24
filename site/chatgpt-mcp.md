# Private ChatGPT connection

Codigest runs on the machine that owns the project. Installing it in a different ChatGPT
execution environment does not grant access to your computer. Public directory submission is
not needed for a private custom MCP connection.

## 1. Install and test

Use a Python release containing `scan_github`, or install the reviewed source branch locally:

```powershell
uv tool install --python 3.15 --editable .
cd PATH_TO_A_SMALL_TEST_PROJECT
codigest mcp --allow-remote
```

The command waits for newline-delimited MCP requests; stop it with Ctrl+C after checking it
starts. Keep the test project small. This server also exposes local tools; remote mode does
not disable local access. Keep the same account, working directory and absolute executable
path when running it under a tunnel or scheduled task.

## 2. Create a private tunnel

Follow https://developers.openai.com/api/docs/guides/secure-mcp-tunnels . In
https://platform.openai.com/settings/organization/tunnels create a tunnel and associate the
intended ChatGPT workspace. Creating needs Tunnels Read + Manage; using needs Read + Use.
Custom MCP availability and tunnel permissions are separate. Check account availability and
billing; this guide does not promise a free service.

Download `tunnel-client` from that settings page. Set `CONTROL_PLANE_API_KEY` locally using
your secure environment or secret manager; never put its value in chat, a command argument,
this repository, or a saved profile. Then, from the test project:

```powershell
tunnel-client help quickstart
tunnel-client init --sample sample_mcp_stdio_local --profile codigest --tunnel-id YOUR_TUNNEL_ID --mcp-command "codigest mcp --allow-remote"
tunnel-client doctor --profile codigest --explain
tunnel-client run --profile codigest
```

If command lookup fails, use the absolute executable path reported by `Get-Command codigest`.
The run process must stay alive. Verify its working directory with the `tree` tool before
reading files; do not assume that the profile preserves your shell directory after restart.

## 3. Connect and verify

At https://chatgpt.com/plugins choose **+ → Add custom MCP server → Connection: Tunnel**,
select the same tunnel and create/install the private plugin. If unavailable, stop and check
account/workspace access rather than replacing this with an unauthenticated public endpoint.

Open a new chat and select the plugin. Check:

1. `tree` returns the intended test project, not a home directory or repository root by accident.
2. `scan` returns a harmless fixture file you created.
3. `scan_github` reads a small public repository/path and reports its resolved commit.
4. Private access is unavailable unless the operator restarted with `--allow-remote --auth`.

Only an observed successful tool call establishes end-to-end ChatGPT access. Local tests and
server startup alone do not. Refresh the plugin connection after changing tools or schemas.
The tunnel supplies connectivity, not autonomous background execution or access to Tailscale.
