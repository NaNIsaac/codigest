# Codigest

[![PyPI](https://img.shields.io/pypi/v/codigest)](https://pypi.org/project/codigest/) [![Tests](https://github.com/NaNIsaac/codigest/actions/workflows/tests.yml/badge.svg)](https://github.com/NaNIsaac/codigest/actions/workflows/tests.yml) [![License: MIT](https://img.shields.io/badge/license-MIT-blue)](LICENSE)

<!-- mcp-name: io.github.NaNIsaac/codigest -->

**Hand an LLM your failing command, the code it points at and your changes, in one paste.**

[한국어](README.ko.md)

<p align="center">
  <img src="docs/demo/demo.gif" alt="cdg cap runs a failing pytest, then cdg pack bundles the failure, the test file it names and the changed file" width="760">
</p>

When a test fails and you ask ChatGPT, Claude or a coding agent for help, you end up copying the
error, then the test, then the file you changed, and the model still asks for one more file.
Codigest does that collecting for you:

1. `cdg cap -- pytest` runs your command as usual and keeps its output.
2. `cdg pack` reads that output, finds the project files it mentions, adds what you changed, and
   copies the result to your clipboard as one block.
3. You paste it into the chat. An agent connected over MCP can fetch the same block itself.

Everything runs locally. Codigest reads your files and Git state; it never modifies your source
files or your Git history.

## Contents

- [Install](#install)
- [Quickstart](#quickstart)
- [What the LLM receives](#what-the-llm-receives)
- [How `pack` chooses files](#how-pack-chooses-files)
- [Concepts](#concepts)
- [Commands](#commands)
- [Terminal capture](#terminal-capture)
- [Using it from an agent (MCP)](#using-it-from-an-agent-mcp)
- [Configuration](#configuration)
- [Privacy and safety](#privacy-and-safety)
- [How it works](#how-it-works)
- [Development](#development)

## Install

Codigest is a command-line tool written in Python. It needs Python 3.15, but you don't have to
install that yourself: [uv](https://docs.astral.sh/uv/) downloads it for the tool and keeps it
separate from your projects. Codigest does not go in your project's dependencies.

```bash
# 1. Install uv if you don't have it
winget install --id astral-sh.uv                    # Windows
curl -LsSf https://astral.sh/uv/install.sh | sh     # macOS/Linux

# 2. Install codigest
uv tool install codigest

# 3. Put uv's tool directory on PATH, then restart your terminal
uv tool update-shell
```

The command is `codigest`, or the short alias `cdg`. To try it without installing:
`uvx codigest tree`.

If `uv` reports a syntax error on `lazy import`, it picked an early 3.15 alpha: run
`uv self update` and install again.

## Quickstart

No setup is needed. Go to any project and run your tests through `cdg cap`:

```bash
cd my-project
cdg cap -- pytest -q        # or npm test, cargo test, go test ./..., make
```

The output appears exactly as it would without `cdg cap`. If the command fails, ask for a pack:

```bash
cdg pack
```

`cdg pack` prints what it chose and copies the block to your clipboard:

```console
$ cdg pack
Packed ~370 / 30,000 tokens
  failed   pytest -q (exit 1, just now)
  changes  uncommitted changes (git)
  + test_prices.py  line 5 in failing output
  + prices.py       changed
📋 Copied pack to clipboard
```

Paste it into the chat and describe what you want, or put the request in the pack itself:
`cdg pack -m "fix the discount calculation"`.

Two optional steps make this smoother:

- `cdg term install` hooks your shell so every command is captured without typing `cdg cap`.
  See [Terminal capture](#terminal-capture).
- `claude mcp add --scope user codigest -- cdg mcp` lets Claude Code (or any MCP client) call
  `pack` without the clipboard. See [Using it from an agent](#using-it-from-an-agent-mcp).

## What the LLM receives

The clipboard holds one XML block. This is the pack from the demo above, unedited apart from the
working directory:

```xml
<context_pack project="project">
<terminal_history>
<command index="1" command="pytest -q" exit_code="1" cwd="~/project" duration="1.5s" at="2026-10-17T01:58:14+09:00">
<output>
F                                                                      [100%]
================================= FAILURES ==================================
_____________________________ test_ten_percent ______________________________

    def test_ten_percent():
&gt;       assert apply_discount(200, 10) == 180
E       assert -1800 == 180
E        +  where -1800 = apply_discount(200, 10)

test_prices.py:5: AssertionError
========================== short test summary info ==========================
FAILED test_prices.py::test_ten_percent - assert -1800 == 180
1 failed in 0.11s
</output>
</command>
</terminal_history>
<changes source="uncommitted changes (git)">
diff --git a/prices.py b/prices.py
index 3d691df..428752a 100644
--- a/prices.py
+++ b/prices.py
@@ -1,2 +1,2 @@
 def apply_discount(price, percent):
-    return price * (1 - percent / 100)
+    return price * (1 - percent)
</changes>
<file path="test_prices.py" reason="line 5 in failing output">
1: from prices import apply_discount
2: 
3: 
4: def test_ten_percent():
5:     assert apply_discount(200, 10) == 180
</file>
<file path="prices.py" reason="changed">
1: def apply_discount(price, percent):
2:     return price * (1 - percent)
</file>
</context_pack>
```

Files carry line numbers so the model can match them to `test_prices.py:5`. Each file says why
it was included. When something did not fit, an `<omitted>` block lists it by name, so the model
knows what it is not seeing and can ask for it.

## How `pack` chooses files

`pack` adds files in this order until the token budget (default 30,000) runs out:

1. Files you name: `cdg pack src/prices.py`.
2. Files named in the output of the last failed command run in this project:
   `File "x.py", line 7` (Python), `x.py:7` (pytest, gcc, rustc, eslint), `x.ts(7,3)` (tsc).
   A failure is dropped once the same command has passed again, so a fixed test does not come back.
3. Changed files: since `cdg mark` if you set a baseline, otherwise uncommitted and untracked
   files from Git.
4. Tests that import a changed Python file (`test_*.py`, `*_test.py`), so the model sees how
   the code is expected to behave.
5. Local Python modules imported by the files above.

The failed command and the diff are always included. Files named like `.env`, `*.pem`, `*.key` or
`id_rsa` are never included. Tokens are estimated at four characters each, so treat the budget
as approximate.

```bash
cdg pack                           # last failure here + your changes
cdg pack -m "fix the discount bug" # add an instruction
cdg pack src/prices.py -b 8000     # add a file, smaller budget
cdg pack -s > context.xml          # print instead of copying
```

If there is no failed command and nothing changed, `pack` says so and suggests what to run.

## Concepts

Three words come up throughout this README.

- **Capture.** A command run through `cdg cap` (or the shell hook) together with its output, exit
  code and working directory. Captures live in one store for the whole machine, not inside your
  project, so they work in any directory.
- **Baseline.** A saved copy of your project's files that `diff`, `semdiff` and `pack` compare
  against. You set it with `cdg mark`. Without one, `pack` uses your uncommitted Git changes
  instead, which is what most people want.
- **Context.** The XML text a command produces for an LLM. Every command that produces context
  copies it to the clipboard; `-s` prints it instead and `--no-copy` skips the clipboard.

## Commands

| Command | What it gives you | Writes to your project |
| --- | --- | --- |
| [`pack`](#how-pack-chooses-files) | Last failure, the files it names, your changes | nothing |
| [`cap`](#terminal-capture) | Runs a command and keeps its output | nothing |
| [`term`](#terminal-capture) | Recent captured commands with output | nothing |
| [`mark`](#mark) | Sets the baseline | `.codigest/` |
| [`diff`](#diff) | Text diff since the baseline | `.codigest/changes.diff` |
| [`semdiff`](#semdiff) | Function and class level changes since the baseline (Python) | `.codigest/semdiff.txt` |
| [`scan`](#scan) | The whole project, or chosen paths, as one block | `.codigest/snapshot.txt` |
| [`digest`](#digest) | Signatures and docstrings, no bodies (Python) | `.codigest/digest.txt` |
| [`tree`](#tree) | Project tree | nothing |
| [`status`](#status) | Baseline age, pending changes, generated files | nothing |
| [`init`](#init) | Config file and baseline | `.codigest/`, `.gitignore` |
| [`clean`](#clean) | Deletes `.codigest/` | removes `.codigest/` |
| [`mcp`](#using-it-from-an-agent-mcp) | MCP server for agents | nothing |

Shared options: `-s` / `--stdout` prints instead of saving, `--no-copy` skips the clipboard,
`-m` / `--message` adds an instruction, and `-r` / `--resolve` (`scan`, `diff`, `semdiff`,
`digest`) also pulls in local files imported by the targets.

### `mark`

Records the current state as the baseline. Run it again to move the baseline.

```bash
cdg mark                # e.g. before a refactor
# ... edit ...
cdg diff                # everything that changed since
```

### `diff`

Text diff of the working tree against the baseline. Your real Git repository is not involved.

```bash
cdg diff
cdg diff -s
```

### `semdiff`

Structural diff for Python files: added, removed and modified functions, methods, classes and
global variables, signature changes and body changes. Formatting and comment changes are ignored.

```bash
cdg semdiff
```

### `scan`

The project tree and file contents as one block. It estimates the size first and asks for
confirmation above 30k tokens or 100 files. `scan` does not move the baseline.

```bash
cdg scan                                   # whole project
cdg scan src/main.py -r                    # one file plus the local files it imports
cdg scan -y --message "Automated snapshot" # no confirmation (CI)
cdg scan -s                                # print instead of saving
```

Other options: `-l` adds line numbers, `-a` ignores the config filters, `-o` sets the output file
name.

### `digest`

The project tree plus the signatures and docstrings of Python files, without function bodies. Use
it to give a model the shape of a large project cheaply.

```bash
cdg digest
```

### `tree`

Prints the project tree, honoring `.gitignore` and the config filters.

### `status`

Shows the baseline's age, files changed since then, and which generated files exist in
`.codigest/`.

### `init`

Writes `.codigest/config.toml` (see [Configuration](#configuration)), sets the baseline like
`mark`, and adds `.codigest/` to `.gitignore`. Only needed if you want to change the file filters.

### `clean`

Deletes `.codigest/`, including the baseline and generated files. Asks first; `-y` skips the
question.

## Terminal capture

`cdg cap` runs a command, streams its output live, and records the command line, combined
stdout/stderr, exit code and working directory. `cdg term` returns recent captures as a
`<terminal_history>` block. Neither needs `cdg init`.

```bash
cdg cap -- pytest -q                # put -- before the command so its flags reach it
cdg cap "npm run build | tail -20"  # quote pipelines

cdg term              # last command
cdg term -n 3         # last 3 commands
cdg term -i 2         # only the command from 2 runs ago
cdg term -f           # last failed command
cdg term --here -n 3  # last 3 commands run in this project

cdg term path         # where captures are stored (override with CODIGEST_HOME)
cdg term clear        # delete all captures
```

Commands not run through `cap` are read from the native shell history (PowerShell, bash, zsh,
fish), without their output.

### Automatic capture

`cdg term install` hooks your shells so every command is captured without the `cap` prefix. It
edits your PowerShell profile, `~/.bashrc` and `~/.zshrc` for whichever of these shells is
installed; choose with `--shell powershell|bash|zsh|all`. On Windows, Git Bash counts as bash.
Commands still run in your current session (`cd`, variables and functions work normally) and
appear in your history as typed. Editors, pagers, REPLs and `ssh` are skipped.

- Skip one command: start the line with a space.
- Skip more programs: `$env:CODIGEST_CAP_SKIP = '^\s*(myrepl|mytui)\b'` (bash/zsh:
  `export CODIGEST_CAP_SKIP='^(myrepl|mytui)( |$)'`, an extended regex).
- Disable for a session: `$env:CODIGEST_NOCAP = 1` (bash/zsh: `export CODIGEST_NOCAP=1`).
- Remove the hooks: `cdg term uninstall`.

Limitations:

- Captured programs write to a pipe, not a TTY, so some lose colors or progress bars.
- In bash (4+) and zsh, multi-line commands (loops typed over several lines, heredocs),
  background jobs (`cmd &`) and lines using `!` history expansion are not captured; they run
  normally. In bash, neither are lines that start with a subshell `( ... )`.
- In bash the hook uses the DEBUG trap and keeps one set before the first prompt (starship,
  bash-preexec); a DEBUG trap set later replaces it.

## Using it from an agent (MCP)

`cdg mcp` serves Codigest as an [MCP](https://modelcontextprotocol.io) server over stdio, so a
coding agent can fetch context itself instead of asking you to paste it.

```bash
# Claude Code
claude mcp add --scope user codigest -- cdg mcp
```

Other clients (Cursor, VS Code, Claude Desktop): add a stdio server with command `cdg` and args
`["mcp"]`.

| Tool | Returns |
| --- | --- |
| `pack` | Last failure, the files it names, changes and their imports, cut to a budget (`targets`, `budget`, `message`) |
| `terminal_history` | Recent commands with output and exit code (`count`, `failed`, `here`: only this project, default on) |
| `scan` | Project snapshot (`targets`, `resolve`, `message`) |
| `diff` / `semdiff` | Text / structural changes since the baseline |
| `digest` | Signatures and docstrings only |
| `tree` | Directory tree |

The tools return the same text the CLI prints. None of them moves the baseline, and paths
outside the project the server was started in are refused.

## Configuration

`cdg init` writes `.codigest/config.toml`. The `[filter]` section controls which files every
command considers:

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

`.git/`, `.codigest/`, virtual environments, `node_modules/` and `__pycache__/` are always
skipped.

## Privacy and safety

- **Secrets in captures.** Known secret shapes (API keys, tokens, `password=...`, private keys,
  URL credentials) are masked before a capture is written to disk. Output of credential commands
  (`env`, `printenv`, `gh auth token`, `aws configure`, ...) is never stored. This catches common
  formats, not every possible secret.
- **Secret files.** `pack` never includes `.env*`, `*.pem`, `*.key`, `*.p12`, `*.pfx` or SSH
  private keys, and lists them as omitted instead.
- **Your project.** No command changes your source files. Commands that save output, and the
  baseline, live only inside `.codigest/`, which `init` adds to `.gitignore`. `pack` without a
  baseline creates nothing at all.
- **Where captures live.** `cdg term path` shows the file. On macOS and Linux it is readable only
  by you. `cdg term clear` deletes it.
- **Nothing leaves your machine.** Codigest makes no network requests. Context goes where you
  paste it, or to the MCP client you connected.

## How it works

- **Baseline.** The baseline is a Git repository at `.codigest/anchor/.shadow_git`, separate from
  your project's repository. The directory is not named `.git` so editors don't mistake it for
  yours. `mark` and `init` commit the current state to it; `diff`, `semdiff`, `status` and `pack`
  compare the working tree against it.
- **Captures.** One newline-delimited JSON file holds the last 200 commands; each output is capped
  at 64 KB, keeping the head and tail.
- **Output.** File contents are XML-escaped inside `<file>` blocks, written flush-left so no stray
  indentation reaches the prompt.
- **MCP.** The server is a small JSON-RPC loop over stdio. Each tool runs the regular CLI with
  `-s --no-copy`, so an agent sees exactly what you would paste.

## Development

```bash
git clone https://github.com/NaNIsaac/codigest
cd codigest
uv run cdg --help              # run from the checkout
uv run --with pytest pytest -q # tests
```

The demo GIF is rendered from recorded output by
[`docs/demo/make_gif.py`](docs/demo/make_gif.py); its docstring explains how to record a new one.

## License

MIT. See [LICENSE](LICENSE).
