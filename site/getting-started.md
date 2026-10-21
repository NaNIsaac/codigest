# Getting Started

Codigest is a command-line tool for the moment you ask ChatGPT, Claude or a coding agent about your code. It turns the part of your project you pick into one block of text you can paste into any chat.

A model only knows what you show it. Copying files by hand is slow, it is easy to miss the one file that matters, and it is just as easy to paste far more than the question needs. Codigest lets you decide what to share and then collects it for you: the whole project, a file and the files it imports, what changed since a point you marked, or a failing test with the code it points at.

[Install it](/installation) first. The command is `cdg`, and no setup is needed in your projects.

In a terminal, every command copies its result to the clipboard. Add `-s` to also print it, `-o FILE` to also write it to a file, `--no-copy` to leave the clipboard alone, and `-m "..."` to put your question or instruction in it. When the output goes to a pipe or a file instead (`cdg scan > ctx.xml`, `cdg digest | less`, a script, a coding agent's shell), it is printed and the clipboard is left alone; add `--copy` to copy it anyway.

## Share your project

Run `scan` in your project and paste:

```bash
cdg scan
```

The clipboard now holds the file tree and every file, honoring `.gitignore`. [Output Formats](/output-formats) shows what that text looks like.

Large projects ask before copying (above 30,000 tokens or 100 files). For those, share less, as below.

## Share only part of it

```bash
cdg scan src/app.py -r       # one file and the local files it imports, recursively
cdg scan src/billing/ docs/  # folders, files, or both
cdg scan src/app.py -l       # with line numbers, so answers can say "line 42"
cdg scan src/app.py:40-80    # only lines 40 to 80, numbered (src/app.py:42 for one line)
cdg digest                   # signatures and docstrings only, much smaller
cdg digest src/billing/      # the same for one folder
cdg tree -L 2                # the file tree, two levels deep
```

`digest` is the overview to start a conversation with. A model that has seen it can ask for exactly what it needs ("show me `Cart.total`"), and `cdg scan prices.py:16-19` gives it just that method, numbered the same way:

```console
$ cdg scan prices.py:16-19
<project_snapshot project="project">
<project_structure>
└── prices.py
</project_structure>
<source_code>
<file path="prices.py" lines="16-19">
16:     def total(self) -> float:
17:         """Sum after discount and tax."""
18:         subtotal = sum(apply_discount(p, self.percent) for p in self.items)
19:         return subtotal * (1 + TAX_RATE)
</file>
</source_code>
</project_snapshot>
```

## Share what changed

Set a baseline first, then work:

```bash
cdg mark                     # e.g. before a refactor
# ... work ...
cdg diff                     # text diff since the mark
cdg semdiff                  # Python: which functions and classes changed
```

A baseline is a private copy kept in `.codigest/`; your Git history is not touched.

## Hand over a failing command

Run the command through `cdg cap`. The output looks exactly as it would without it; Codigest keeps a copy. Then ask for a pack:

```bash
cdg cap -- pytest -q        # or npm test, cargo test, go test ./..., make
cdg pack
```

```console
$ cdg pack
Packed ~689 / 30,000 tokens
  failed   pytest -q (exit 1, just now)
  changes  uncommitted changes (git)
  + test_prices.py     line 5 in failing output
  + prices.py          called from failing test test_prices.py
  + legacy/discount.m  changed
```

Read it as: the failing command, your uncommitted Git changes, `test_prices.py` because the error points at its line 5, `prices.py` because that test calls code in it, and `legacy/discount.m` because you edited it. Files that did not fit the budget would be listed in an `<omitted>` block, so the model knows what it is not seeing.

### How `pack` decides

Plain `cdg pack` fills its token budget (default 30,000) in this order:

1. Files named in the output of the last failed command run in this project in the past hour: `File "x.py", line 7` (Python), `x.py:7` (pytest, ruff, gcc, rustc, eslint), `x.ts(7,3)` (tsc). Once the same command passes again, that failure is no longer used. A file that is only named in the output, not changed by you, comes as an excerpt: ten lines around each place the output points at, marked with a `lines="40-60"` attribute. Short files come whole.
2. The project functions and classes that the failing Python tests call, whole, with the reason `called from failing test ...`. A traceback that ends in an assertion names only the test; this brings the code under test even when the bug is already committed and nothing shows in a diff.
3. Files you changed: uncommitted and untracked files in Git, or changes since `cdg mark`. A long changed file comes as the parts its diff hunks touch, since the diff itself is in the pack already.
4. Tests that import those files (`test_*.py`, `*_test.py`, `*.test.ts`, `*.spec.js`, ...).
5. Project files they import: Python imports, and relative imports in JS/TS (`./x`, `./x.js` for `x.ts`, `./dir` for `dir/index.ts`).

When you name files, you get only those files. `--with` adds automatic sources back:

```bash
cdg pack src/prices.py                         # just this file
cdg pack src/prices.py --with tests            # this file and its tests
cdg pack --with changes                        # only your changes, no failure
cdg pack --with failure,changes,tests,imports  # the same as plain `cdg pack`
```

The failed command and the diff are always included when their source is on. A long command output keeps its first 80 and last 40 lines (the first failures and the summary); `cdg term -f` shows all of it. Tokens are estimated at four characters each, so the budget is approximate.

`cdg status` shows what `pack` would use right now without building anything.

### Capture without typing `cap`

`cdg term install` hooks your shell so every command is captured without typing `cdg cap` (`cdg term uninstall` removes it). See [Terminal Capture](/terminal-capture).

## Let an agent fetch it

Connect your coding agent over MCP and it requests context itself instead of asking you to paste:

```bash
claude mcp add --scope user codigest -- cdg mcp      # Claude Code
```

See [MCP Integration](/mcp) for other clients. An agent that can run shell commands needs no setup at all. Its shell is not a terminal, so `cdg` prints the result for it and never touches your clipboard. A way of working that suits agents:

```bash
cdg tree -L 2                      # where things are
cdg digest src/                    # what is defined where, with line numbers
cdg scan src/prices.py:16-19      # read the parts that matter, numbered the same way
cdg mark                           # before editing
cdg semdiff                        # afterwards: which functions it changed
cdg cap -- pytest -q || cdg pack   # run the tests; on failure, the pack has what to fix
```

## Where to go next

- [Command Reference](/commands) for every command and option.
- [Recipes](/recipes) for complete workflows.
- [Security](/security) for what is masked and what is never included.
