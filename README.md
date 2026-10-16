# Codigest

[![PyPI](https://img.shields.io/pypi/v/codigest)](https://pypi.org/project/codigest/) [![Tests](https://github.com/NaNIsaac/codigest/actions/workflows/tests.yml/badge.svg)](https://github.com/NaNIsaac/codigest/actions/workflows/tests.yml) [![License: MIT](https://img.shields.io/badge/license-MIT-blue)](LICENSE)

<!-- mcp-name: io.github.NaNIsaac/codigest -->

**Hand an LLM your failing command, the code it points at and your changes, in one paste.**

[한국어](README.ko.md)

<p align="center">
  <img src="docs/demo/demo.gif" alt="cdg cap runs a failing pytest, then cdg pack bundles the failure, the test file it names and the changed file" width="760">
</p>

## The idea

A test fails and you ask an LLM for help. You copy the error, then the test, then the file you
changed, and the model still asks for one more file.

Codigest does the copying. You need two commands:

```text
cdg cap -- pytest     runs your command and remembers its output
cdg pack              finds the files that output mentions, adds what you changed,
                      and copies all of it to your clipboard
```

Then you paste. That is the whole tool for most days; everything below either sets it up,
explains it, or is optional.

## Install

Codigest is a command-line tool. Install it with [uv](https://docs.astral.sh/uv/), which also
fetches the Python version it needs (3.15) without touching your projects:

```bash
winget install --id astral-sh.uv                  # Windows (skip if you have uv)
curl -LsSf https://astral.sh/uv/install.sh | sh   # macOS/Linux

uv tool install codigest
uv tool update-shell                              # then restart your terminal
```

The command is `codigest`, or `cdg` for short.
If the install fails with a syntax error on `lazy import`, run `uv self update` and try again.

## Your first pack

Go to a project with a failing test. No setup is needed.

**1. Run the test through `cdg cap`.** Put `--` before your command. The output looks exactly as
it would without `cdg cap`; Codigest just keeps a copy.

```bash
cdg cap -- pytest -q        # or npm test, cargo test, go test ./..., make
```

**2. Run `cdg pack`.** It tells you what it picked and why:

```console
$ cdg pack
Packed ~370 / 30,000 tokens
  failed   pytest -q (exit 1, just now)
  changes  uncommitted changes (git)
  + test_prices.py  line 5 in failing output
  + prices.py       changed
📋 Copied pack to clipboard
```

Read it as: the failing command, your uncommitted Git changes, `test_prices.py` because the error
points at its line 5, and `prices.py` because you edited it.

**3. Paste into the chat** and say what you want. You can also put the request in the pack:
`cdg pack -m "fix the discount calculation"`.

<details>
<summary>What exactly was copied</summary>

One XML block: the command with its output, the diff, and each file with line numbers and the
reason it was included. This is the pack above, unedited apart from the working directory:

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

Files that did not fit the budget would be listed in an `<omitted>` block, so the model knows
what it is not seeing.

</details>

## Two optional upgrades

Once the basic loop works, each of these removes one manual step.

**Stop typing `cdg cap`.** Hook your shell once and every command is captured automatically:

```bash
cdg term install            # cdg term uninstall to undo
```

**Stop pasting.** Connect your coding agent over MCP and it fetches the pack itself:

```bash
claude mcp add --scope user codigest -- cdg mcp      # Claude Code
```

Other clients (Cursor, VS Code, Claude Desktop): add a stdio server with command `cdg` and args
`["mcp"]`. Details for both are in the [reference](#automatic-capture).

## How `pack` decides

When a file you expected is missing, or one you didn't expect is there, this is the order `pack`
fills its token budget (default 30,000):

1. Files you name: `cdg pack src/prices.py`.
2. Files named in the output of the last failed command run in this project in the past hour:
   `File "x.py", line 7` (Python), `x.py:7` (pytest, ruff, gcc, rustc, eslint), `x.ts(7,3)` (tsc).
   Once the same command passes again, that failure is no longer used. A file that is only named
   in the output, not changed by you, comes as an excerpt: ten lines around each place the output
   points at, marked with a `lines="40-60"` attribute. Short files come whole.
3. Files you changed: uncommitted and untracked files in Git, or changes since `cdg mark` if you
   set a baseline (see [below](#see-what-changed-since-a-point)).
4. Tests that import a changed file (`test_*.py`, `*_test.py`, `*.test.ts`, `*.spec.js`, ...).
5. Project files imported by the files above: Python imports, and relative imports in
   JS/TS (`./x`, `./x.js` for `x.ts`, `./dir` for `dir/index.ts`).

The failed command and the diff are always included. `.env`, `*.pem`, `*.key` and SSH keys never
are. Tokens are estimated at four characters each, so the budget is approximate.

```bash
cdg pack -b 8000            # smaller budget
cdg pack -s > context.xml   # print instead of copying
```

## Other things you can hand an LLM

`pack` covers the failing-test case. These cover the rest, grouped by what you want to do.

### Recall an earlier command

Every captured command stays available, not only the last failure:

```bash
cdg term              # last command and its output
cdg term -n 3         # last 3
cdg term -f           # last failed one
cdg term --here -n 3  # last 3 run in this project
```

### See what changed since a point

Set a baseline before you start, then ask for everything since:

```bash
cdg mark              # e.g. before a refactor
# ... work ...
cdg diff              # text diff since the mark
cdg semdiff           # Python only: which functions and classes changed
```

A baseline is a private copy kept in `.codigest/`; your Git history is not touched. Once you set
one, `pack` also uses it instead of `git diff`.

### Give the whole project, or its shape

```bash
cdg scan              # every file, as one block (asks first above 30k tokens)
cdg scan src/app.py -r  # one file plus the local files it imports
cdg digest            # Python signatures and docstrings only, much smaller
cdg tree              # just the file tree
```

## Reference

- [All commands](#all-commands)
- [Terminal capture](#terminal-capture)
- [MCP tools](#mcp-tools)
- [Configuration](#configuration)
- [Privacy and safety](#privacy-and-safety)
- [How it works](#how-it-works)
- [Development](#development)

### All commands

Every command that produces context copies it to the clipboard. `-s` / `--stdout` prints it
instead, `--no-copy` skips the clipboard, and `-m` / `--message` adds an instruction.

| Command | What it gives you | Writes to your project |
| --- | --- | --- |
| `pack` | Last failure, the files it names, your changes | nothing |
| `cap` | Runs a command and keeps its output | nothing |
| `term` | Recent captured commands with output | nothing |
| `mark` | Sets the baseline | `.codigest/` |
| `diff` | Text diff since the baseline | `.codigest/changes.diff` |
| `semdiff` | Function and class level changes since the baseline (Python) | `.codigest/semdiff.txt` |
| `scan` | The whole project, or chosen paths, as one block | `.codigest/snapshot.txt` |
| `digest` | Signatures and docstrings, no bodies (Python) | `.codigest/digest.txt` |
| `tree` | Project tree | nothing |
| `status` | Baseline age, pending changes, generated files | nothing |
| `init` | Config file and baseline; only needed to change [file filters](#configuration) | `.codigest/`, `.gitignore` |
| `clean` | Deletes `.codigest/` (asks first, `-y` to skip) | removes `.codigest/` |
| `mcp` | MCP server for agents | nothing |

More options: `scan -l` adds line numbers, `scan -a` ignores the config filters, `scan -y` skips
the confirmation (CI). `-r` / `--resolve` on `scan`, `diff`, `semdiff` and `digest` also pulls in
local files imported by the targets. `cdg <command> --help` lists everything.

### Terminal capture

`cdg cap` streams the command's output live and records the command line, combined
stdout/stderr, exit code and working directory. Captures from all projects share one store on
your machine, so `cap` and `term` work in any directory.

```bash
cdg cap "npm run build | tail -20"  # quote pipelines
cdg term -i 2         # only the command from 2 runs ago
cdg term path         # where captures are stored (override with CODIGEST_HOME)
cdg term clear        # delete all captures
```

Commands not run through `cap` are read from the native shell history (PowerShell, bash, zsh,
fish), without their output.

#### Automatic capture

`cdg term install` edits your PowerShell profile, `~/.bashrc` and `~/.zshrc` for whichever of
these shells is installed; choose with `--shell powershell|bash|zsh|all`. On Windows, Git Bash
counts as bash. Commands still run in your current session (`cd`, variables and functions work
normally) and appear in your history as typed. Editors, pagers, REPLs and `ssh` are skipped.

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

### MCP tools

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

### Configuration

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

### Privacy and safety

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

### How it works

- **Baseline.** A Git repository at `.codigest/anchor/.shadow_git`, separate from your project's
  repository. The directory is not named `.git` so editors don't mistake it for yours. `mark` and
  `init` commit the current state to it; `diff`, `semdiff`, `status` and `pack` compare the
  working tree against it.
- **Captures.** One newline-delimited JSON file holds the last 200 commands; each output is capped
  at 64 KB, keeping the head and tail.
- **Output.** File contents are XML-escaped inside `<file>` blocks, written flush-left so no stray
  indentation reaches the prompt.
- **MCP.** The server is a small JSON-RPC loop over stdio. Each tool runs the regular CLI with
  `-s --no-copy`, so an agent sees exactly what you would paste.

### Development

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
