# Installation

## Prebuilt binary (recommended)

One native binary, no Python needed. Windows (PowerShell):

```powershell
irm https://raw.githubusercontent.com/NaNIsaac/codigest/main/install.ps1 | iex
```

Linux, macOS and Termux:

```bash
curl -fsSL https://raw.githubusercontent.com/NaNIsaac/codigest/main/install.sh | sh
```

The script downloads the latest release for your system and installs `cdg`: on Windows to `%LOCALAPPDATA%\Programs\codigest`, which it adds to your user `PATH`; on Linux and macOS to `~/.local/bin`; in Termux to `$PREFIX/bin`. Open a new terminal afterwards. Run the same line again to update.

Supported: Windows x64, Linux x64 and ARM64 (static, musl), macOS Intel and Apple Silicon, and Android ARM64 (Termux).

Two environment variables change what the script does. `CDG_VERSION=0.12.1` installs that version instead of the latest. `CDG_INSTALL_DIR=...` installs into that folder, and then `PATH` is left alone.

```powershell
$env:CDG_VERSION = "0.12.1"; irm https://raw.githubusercontent.com/NaNIsaac/codigest/main/install.ps1 | iex
```

```bash
curl -fsSL https://raw.githubusercontent.com/NaNIsaac/codigest/main/install.sh | CDG_INSTALL_DIR=/opt/codigest sh
```

To download the archive yourself, see [Rust Port](/rust-port#install). That page also has the speed numbers and the few differences from the Python package.

The command is `cdg`. No setup is needed in your projects.

Check that it works:

```bash
cdg --version
cdg --help
```

## Python package

The same commands are also published on PyPI as `codigest`. Install it with [uv](https://docs.astral.sh/uv/), which also fetches the Python version it needs (3.15) and keeps it apart from your projects. pip and pipx work too if you already have that Python.

```bash
winget install --id astral-sh.uv                  # Windows (skip if you have uv)
curl -LsSf https://astral.sh/uv/install.sh | sh   # macOS/Linux

uv tool install codigest
uv tool update-shell                              # then restart your terminal
```

The command is `codigest`, or `cdg` for short.

::: warning Keep one `cdg` on your PATH
The binary and the Python package both install a command called `cdg`, and whichever comes first on `PATH` runs. Use one of them. The MCP registry entry runs the Python package (`uvx codigest mcp`), which does not put anything on `PATH`.
:::

::: tip If the install fails with a syntax error on `lazy import`
The Python package uses Python 3.15 syntax, and an old uv may not know how to fetch that Python. Run `uv self update` and install again.
:::

## PowerShell help

On Windows, PowerShell's `help codigest` is a separate command from `codigest --help`. For optional native `Get-Help` integration **without automatic command capture**:

```powershell
cdg shell-help install       # register comment-based PowerShell help
# open a new PowerShell window
help codigest                # or: Get-Help codigest
cdg shell-help uninstall     # remove it when no longer needed
```

This adds a small forwarding function to your PowerShell profile; other `help` commands are unchanged. The CLI's own full command reference is still available through `codigest --help`.

## Next: capture every command

`cdg term install` hooks your shell so the output of every command is kept for `cdg pack` and `cdg term`. It is off until you run it. See [Terminal Capture](/terminal-capture).

## Run from a checkout

To try a change to Codigest itself without installing it:

```bash
git clone https://github.com/NaNIsaac/codigest
cd codigest
uv run cdg --help
```

## Uninstall

Remove the shell hooks first if you installed them, then the tool:

```bash
cdg term uninstall
cdg shell-help uninstall     # PowerShell only, if you used it
```

Then delete the binary: the folder `%LOCALAPPDATA%\Programs\codigest` on Windows (and its entry in your user `PATH`), or `~/.local/bin/cdg` (`$PREFIX/bin/cdg` in Termux). For the Python package, run `uv tool uninstall codigest` instead.

Captured commands live outside your projects; `cdg term clear` deletes them. A project's `.codigest/` folder is removed with `cdg clean`. See [Architecture](/architecture#where-things-are-stored) for the locations.
