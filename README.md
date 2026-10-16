# Codigest

[![PyPI](https://img.shields.io/pypi/v/codigest)](https://pypi.org/project/codigest/) [![Tests](https://github.com/NaNIsaac/codigest/actions/workflows/tests.yml/badge.svg)](https://github.com/NaNIsaac/codigest/actions/workflows/tests.yml) [![License: MIT](https://img.shields.io/badge/license-MIT-blue)](LICENSE)

<!-- mcp-name: io.github.NaNIsaac/codigest -->

**Hand an LLM your failing command, your recent changes or your whole codebase as one block of XML.**

A test fails. Run it through `cdg cap`, then `cdg pack` puts the failed command, the files its output
points at and your uncommitted changes on your clipboard, ready to paste into a chat:

```console
$ cdg cap -- python -m unittest -q
======================================================================
FAIL: test_ten_percent (test_prices.DiscountTest.test_ten_percent)
----------------------------------------------------------------------
Traceback (most recent call last):
  File "~/project/test_prices.py", line 7, in test_ten_percent
    self.assertEqual(apply_discount(200, 10), 180)
    ~~~~~~~~~~~~~~~~^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
AssertionError: -1800 != 180

----------------------------------------------------------------------
Ran 1 test in 0.001s

FAILED (failures=1)
$ cdg pack
Packed ~415 / 30,000 tokens
  failed   python -m unittest -q (exit 1, just now)
  changes  uncommitted changes (git)
  + test_prices.py  line 7 in failing output
  + prices.py       changed
📋 Copied pack to clipboard
```

The clipboard holds one `<context_pack>` block: the command with its output and exit code, the
`git diff`, and both files with line numbers. Anything that did not fit the budget is listed by name.
An agent connected over MCP (`cdg mcp`) gets the same block from the `pack` tool, with no copying.

Try it on your own project. `tree` only reads:

```console
$ uvx codigest tree
project
├── .gitignore (26 bytes)
├── prices.py (102 bytes)
└── test_prices.py (227 bytes)

Found 3 files.
```

Codigest is a CLI and MCP server that turns your codebase, your recent changes and your terminal output into XML context for LLMs.

`cdg mark` can also keep a private baseline (a hidden Git repository inside `.codigest/`) so it can report what changed since that point without touching your real Git history. It never modifies your source files. The command is `codigest`, or the short alias `cdg`.

## Quickstart

```bash
uvx codigest tree          # try it without installing
uv tool install codigest  # or install the `cdg` command

cdg cap -- pytest -q      # run a command and keep its output
cdg pack                  # failure + the files it names + your changes, to the clipboard
cdg scan                  # whole project as LLM-ready XML
cdg mark && cdg diff      # set a baseline, later get only what changed
claude mcp add codigest -- cdg mcp   # let AI agents call all of the above
```

## Install

Codigest needs Python 3.15 and is meant to be installed as a global tool with [uv](https://docs.astral.sh/uv/), which downloads the right Python for it. It does not go in your project's dependencies.

```bash
# 1. Install uv if you don't have it
winget install --id astral-sh.uv                    # Windows
curl -LsSf https://astral.sh/uv/install.sh | sh     # macOS/Linux

# 2. Install codigest
uv tool install codigest

# 3. Put uv's tool directory on PATH, then restart your terminal
uv tool update-shell
```

If `uv` reports a syntax error on `lazy import`, it picked an early 3.15 alpha: run `uv self update` and install again.

## Commands

Every command that produces context copies it to the clipboard by default (`--no-copy` to skip) and prints it to the terminal with `-s` / `--stdout`. `-m` / `--message` adds an instruction to the output, and `-r` / `--resolve` also pulls in local files imported by the targets.

### `pack`

Builds one block for the task at hand, adding files in this order until the token budget (default 30,000, estimated at 4 characters per token) runs out:

1. files you name (`cdg pack src/prices.py`)
2. files named in the output of the last failed command run in this project (`File "x.py", line 7`, `x.py:7`, `x.ts(7,3)`). A failure is skipped once the same command has passed again.
3. changed files: since `cdg mark` if you set one, otherwise uncommitted and untracked files from Git
4. local Python modules imported by those files

The failed command and the diff are always included. Files named like `.env`, `*.pem`, `*.key` or `id_rsa` are never included. Skipped files are listed in an `<omitted>` block with the reason. `pack` writes nothing to the project.

```bash
cdg pack
cdg pack -m "fix the discount bug"
cdg pack src/prices.py -b 8000
cdg pack -s > context.xml
```

### `mark`

Records the current state as the baseline for `diff`, `semdiff` and `pack`. Run it again to move the baseline.

```bash
cdg mark
```

### `init`

Creates `.codigest/` (config and baseline), records the current state as the baseline like `mark`, and adds `.codigest/` to `.gitignore`.

```bash
cdg init
```

### `scan`

Writes the project tree and file contents as XML to `.codigest/snapshot.txt`. It does not move the baseline (use `mark`). It estimates the token count first and asks for confirmation above 30k tokens.

```bash
cdg scan                              # whole project
cdg scan src/main.py -r               # one file plus the local files it imports
cdg scan -y --message "Automated snapshot"   # no confirmation (CI)
cdg scan -s                           # print instead of saving
```

Other options: `-l` adds line numbers, `-a` ignores the config filters, `-o` sets the output file name.

### `diff`

Text diff of the working tree against the baseline (the last `mark` or `init`), saved to `.codigest/changes.diff`. Your real Git repository is not involved.

```bash
cdg diff
cdg diff -s
```

### `semdiff`

Structural diff for Python files: added, removed and modified functions, methods, classes and global variables, signature changes and body changes. Formatting and comment changes are ignored. Saved to `.codigest/semdiff.txt`.

```bash
cdg semdiff
cdg semdiff -s
```

### `digest`

Project tree plus the signatures and docstrings of Python files, without function bodies. Saved to `.codigest/digest.txt`.

```bash
cdg digest
cdg digest -s
```

### `tree`

Prints the project tree, honoring `.gitignore` and the config filters.

```bash
cdg tree
```

### `status`

Shows the state of `.codigest/`: anchor baseline, files changed since then, and generated files.

```bash
cdg status
```

### `clean`

Deletes `.codigest/`, including the anchor and generated files.

```bash
cdg clean        # asks first
cdg clean -y
```

### `cap` and `term`: terminal capture

`cap` runs a command (output still streams live) and records the command line, combined stdout/stderr, exit code and working directory in a store shared by all projects. `term` returns recent entries as a `<terminal_history>` block for your LLM. Neither needs `cdg init`.

- Known secret shapes (API keys, tokens, `password=...`) are redacted before anything is written. Output of credential commands (`env`, `printenv`, `gh auth token`, ...) is never stored.
- Commands not run through `cap` are read from the native shell history (PowerShell, bash, zsh, fish), without output.

```bash
cdg cap -- pytest -q                # prefix with -- so flags reach the command
cdg cap "npm run build | tail -20"

cdg term              # last command
cdg term -n 3         # last 3 commands
cdg term -i 2         # only the command from 2 runs ago
cdg term -f           # last failed command
cdg term --here -n 3  # last 3 commands run in this project

cdg term path         # where captures are stored (override with CODIGEST_HOME)
cdg term clear        # delete all captures
```

**Automatic capture.** `cdg term install` hooks your shells so every command is captured without the `cap` prefix. It edits your PowerShell profile, `~/.bashrc` and `~/.zshrc` for whichever of these shells is installed; choose with `--shell powershell|bash|zsh|all`. On Windows, Git Bash counts as bash. Commands still run in your current session (`cd`, variables and functions work normally) and appear in your history as typed. Editors, pagers, REPLs and `ssh` are skipped.

- Skip one command: start the line with a space.
- Skip more programs: `$env:CODIGEST_CAP_SKIP = '^\s*(myrepl|mytui)\b'` (bash/zsh: `export CODIGEST_CAP_SKIP='^(myrepl|mytui)( |$)'`, an extended regex).
- Disable for a session: `$env:CODIGEST_NOCAP = 1` (bash/zsh: `export CODIGEST_NOCAP=1`).
- Remove the hooks: `cdg term uninstall`.
- Captured programs write to a pipe, not a TTY, so some lose colors or progress bars.
- In bash (4+) and zsh, multi-line commands (loops typed over several lines, heredocs), background jobs (`cmd &`) and lines using `!` history expansion are not captured; they run normally. In bash, neither are lines that start with a subshell `( ... )`.
- In bash the hook uses the DEBUG trap and keeps one set before the first prompt (starship, bash-preexec); a DEBUG trap set later replaces it.

### `mcp`: for AI agents

`cdg mcp` serves Codigest as an [MCP](https://modelcontextprotocol.io) server over stdio, so coding agents can fetch context themselves.

| Tool | Returns |
| --- | --- |
| `pack` | Last failure, the files it names, changes and their imports, cut to a budget (`targets`, `budget`, `message`) |
| `terminal_history` | Recent commands with output and exit code (`count`, `failed`, `here`: only this project, default on) |
| `scan` | Project snapshot (`targets`, `resolve`, `message`) |
| `diff` / `semdiff` | Text / structural changes since the `mark` baseline |
| `digest` | Signatures and docstrings only |
| `tree` | Directory tree |

```bash
# Claude Code
claude mcp add --scope user codigest -- cdg mcp
```

Other clients (Cursor, VS Code, Claude Desktop): add a stdio server with command `cdg` and args `["mcp"]`.

## Configuration

`cdg init` writes `.codigest/config.toml`. The `[filter]` section controls which files are included:

```toml
[filter]
# Only these extensions (omit to include every non-ignored file)
extensions = [".py", ".ts", ".rs", ".md", ".json"]

# Extra exclusions, in .gitignore syntax (.gitignore itself is always honored)
exclude_patterns = [
    "*.lock",
    "tests/data/",
    "legacy_code/"
]
```

## How it works

- **Baseline.** The baseline is a Git repository at `.codigest/anchor/.shadow_git`. The directory is not named `.git` so that editors don't mistake it for your project's repository. `mark` and `init` commit the current state to it; `diff`, `semdiff`, `status` and `pack` compare the working tree against it.
- **Output.** File contents are XML-escaped inside `<file>` blocks, and the blocks are written flush-left so no stray indentation reaches the prompt.

## License

MIT. See [LICENSE](LICENSE).
