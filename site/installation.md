# Installation

Install Codigest with [uv](https://docs.astral.sh/uv/), which also fetches the Python version Codigest needs (3.15) and keeps it apart from your projects.

```bash
winget install --id astral-sh.uv                  # Windows (skip if you have uv)
curl -LsSf https://astral.sh/uv/install.sh | sh   # macOS/Linux

uv tool install codigest
uv tool update-shell                              # then restart your terminal
```

The command is `codigest`, or `cdg` for short. No setup is needed in your projects.

Check that it works:

```bash
cdg --version
cdg --help
```

::: tip If the install fails with a syntax error on `lazy import`
Codigest uses Python 3.15 syntax, and an old uv may not know how to fetch that Python. Run `uv self update` and install again.
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

## Optional: capture every command

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
uv tool uninstall codigest
```

Captured commands live outside your projects; `cdg term clear` deletes them. A project's `.codigest/` folder is removed with `cdg clean`. See [Architecture](/architecture#where-things-are-stored) for the locations.
