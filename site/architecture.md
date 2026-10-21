# Architecture

Codigest is a Python package (`codigest`, installed as the `codigest` and `cdg` commands). It is a [Typer](https://typer.tiangolo.com/) app: each command is a small module that gathers input, calls into a shared core, and hands one block of text to an output handler.

## Layout

```text
src/codigest/
├── main.py          command registration, --version, top-level error handling
├── options.py       the shared options (-s, --copy, -o, -m, -r, -a)
├── commands/        one module per command
│   ├── scan.py  digest.py  tree.py
│   ├── mark.py  diff.py  semdiff.py
│   ├── cap.py  term.py  pack.py  status.py
│   ├── mcp.py
│   └── init.py  clean.py  shell_help.py
├── core/
│   ├── common.py      project root, config filters, file selection
│   ├── scanner.py     walks the tree, applies .gitignore and the filters
│   ├── resolver.py    follows imports (Python, relative JS/TS)
│   ├── processor.py   reads files, line numbers, line ranges
│   ├── outline.py     digest: signatures and docstrings (Python, MATLAB)
│   ├── semdiff.py     function and class level comparison (Python AST)
│   ├── shadow.py      the baseline ("anchor") behind mark, diff, semdiff
│   ├── git_ops.py     read-only Git queries for pack
│   ├── callees.py     the project code a failing test calls
│   ├── termstore.py   the capture store and its XML rendering
│   ├── runner.py      runs a command for cap, streaming its output
│   ├── shellhist.py   native shell history readers
│   ├── redact.py      masking, denied commands, secret file names
│   ├── prompts.py     default output wrappers and prompts.toml templates
│   ├── tags.py        <file> blocks and escaping
│   ├── output.py      clipboard, stdout, -o, terminal vs pipe
│   ├── cache.py       per-file block and digest cache
│   ├── structure.py   the ASCII tree
│   └── tokenizer.py   the token estimate
└── shell/             hook.ps1, hook.sh, help.ps1
```

Heavy imports are lazy (Python 3.15 `lazy import`), so `cdg --help` and short commands start fast.

## Data flow

Most commands follow the same path:

1. **Find the project.** `common.get_context` starts at the current folder (or the common parent of the paths you named) and goes up to the first folder containing `.codigest/` or `.git/`. That is the root. It also reads the `[filter]` section of `.codigest/config.toml`.
2. **Select files.** `scanner` walks the root, skipping the always-ignored folders, `.gitignore` and `exclude_patterns`, and keeping only `extensions` if set. `resolver` adds imported files when `-r` is given. Credential files are removed here and remembered, so the command can say it withheld them.
3. **Build the content.** `scan` reads files (with line numbers or ranges), `digest` outlines them, `diff` and `semdiff` compare them with the baseline, `pack` ranks files by why they matter, `term` renders captures.
4. **Wrap it.** `prompts` puts the content in its root element, or in your template from `prompts.toml`. `tags.file` builds each `<file>` block with the code verbatim.
5. **Output.** `OutputHandler` masks unambiguous secrets once more, then copies, writes `-o`, and prints according to whether stdout is a terminal.

`cap` is the exception: it runs a program through `runner`, mirrors its output live, and appends a record to the capture store through `termstore`, which masks and trims it first. `pack` reads that store, then follows the same flow as above.

## Where things are stored

**In the project, under `.codigest/`:**

| Path | What |
| --- | --- |
| `config.toml` | File filters. Written by `cdg init`. |
| `prompts.toml` | Your templates. Written by you. |
| `anchor/` | The baseline: a copy of the project's files. |
| `anchor/.shadow_git` | A Git repository for the baseline, separate from your project's. |
| `cache.json` | Cached `scan` blocks and `digest` summaries, keyed by file content. Only written in projects that already have `.codigest/`. |
| `.gitignore` | Contains `*`, so Git never sees any of this. |

`cdg clean` removes the folder.

**On the machine, outside any project:**

| Platform | Data directory |
| --- | --- |
| Windows | `%LOCALAPPDATA%\codigest` |
| macOS, Linux | `$XDG_DATA_HOME/codigest`, or `~/.local/share/codigest` |
| any | `$CODIGEST_HOME`, when set |

Captures are in `term/history.jsonl` inside it, with `term/ingest-errors.log` next to it if a hook ever failed. `cdg term path` prints the exact file. `cdg term install` also copies the shell hooks to `shell/` in the same folder, and the startup files load those copies.

## The anchor

`cdg mark` sets the baseline. It copies the files the commands currently consider into `.codigest/anchor/` and commits them to a shadow Git repository whose Git directory is `.codigest/anchor/.shadow_git`. Your project's own repository is never read or written by this.

- `diff` compares the working tree against the last commit there.
- `semdiff` finds the files whose text changed, then compares the old and new versions' syntax trees.
- `status` and `pack` use the anchor if it exists and your Git working tree otherwise.

Running `mark` again takes a new commit, which is how the baseline moves. If nothing changed, nothing is committed. The shadow repository is named `.shadow_git`, not `.git`, so editors do not pick it up as a nested repository.

## The capture store

One newline-delimited JSON file holds the last 200 commands. Each output is capped at 64 KB, keeping the head and tail. Entries from the shell hooks arrive from background processes and may land out of order, so they are sorted by start time when read. There is no file lock; concurrent ingests can in rare cases drop an entry.

## MCP

The server is a small JSON-RPC loop over stdio. Each tool runs the regular CLI (`python -m codigest <command> -s --no-copy`) as a subprocess, so an agent sees exactly what you would paste, and stdout stays clean for the protocol. See [MCP Integration](/mcp).

## Output rules

File contents go into `<file>` blocks verbatim. Only a closing tag of one of Codigest's own blocks inside a file (a literal `</file>`) is written as `&lt;/file>`, so it cannot end the block early. See [Output Formats](/output-formats).
