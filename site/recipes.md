# Recipes

Complete workflows, each from the situation to the paste. The examples use the small project from [Getting Started](/getting-started): `prices.py`, `test_prices.py`, and a `legacy/` folder.

## A test fails and you do not know why

Run the tests through `cap`, then pack:

```bash
cdg cap -- pytest -q
cdg pack -m "Why does test_ten_percent fail?"
```

The output of `cap` looks the same as a plain `pytest -q`. `pack` collects the failed command with its output, your uncommitted changes, the test file the traceback names, and the code that test calls:

```console
$ cdg pack
Packed ~689 / 30,000 tokens
  failed   pytest -q (exit 1, just now)
  changes  uncommitted changes (git)
  + test_prices.py     line 5 in failing output
  + prices.py          called from failing test test_prices.py
  + legacy/discount.m  changed
```

Paste it into the chat. If the bug is already committed and nothing shows in a diff, `prices.py` is still there because the failing test calls it. After you fix the code, run `cdg cap -- pytest -q` again: once the same command passes, that failure is no longer used by `pack`.

If the pack is larger than you want, lower the budget (`-b 8000`) or name what you need: `cdg pack prices.py test_prices.py`.

## A long failure, only the useful part

A command with thousands of lines of output keeps its first 80 and last 40 lines in the pack, which is where the first failures and the summary are. A file that is only named in the output, and that you did not change, comes as ten lines around each place the output points at. Both keep the pack small without choosing for you.

To see everything that was captured, use `cdg term -f`.

## Review a refactor with mark and semdiff

Set a baseline before you start. It does not touch Git, so it works on a dirty tree and in a folder without a repository.

```bash
cdg mark
# ... refactor ...
cdg semdiff -m "Review these signature changes for callers I may have missed"
```

`semdiff` answers "what did I actually change" at the level of functions and classes:

```console
$ cdg semdiff
Analyzing structural changes since (1 second ago)...
SemDiff Generated! (~70 Tokens)
<semantic_diff project="project" source="since cdg mark (1 second ago)">
<file path="prices.py">
[SIGNATURE] 16: method Cart.total :: (self) -> float -> (self, tax: bool=True) -> float
[LOGIC]     4: function apply_discount
[ADDED]     22: function tax_of
</file>
</semantic_diff>
```

A `[SIGNATURE]` line is where callers can break, so it is usually the part worth a second opinion. When the model asks to see a body, send just that: `cdg scan prices.py:4-6`. For the full text change, use `cdg diff`.

`semdiff` reads Python. For other languages use `cdg diff`. `cdg mark` again moves the baseline to now.

## Share one folder or one question's worth of files

Start small and widen only when asked:

```bash
cdg tree -L 2                     # the shape of the project
cdg digest src/billing/           # what is defined where, no bodies
cdg scan src/billing/invoice.py   # the file the question is about
cdg scan src/billing/invoice.py -r  # ... and the local files it imports
cdg scan src/billing/ docs/ -l    # a folder and a docs folder, numbered
```

`digest` is a good first message: it is much smaller than the code, and the model can then ask for `Cart.total` by name. You answer with `cdg scan prices.py:16-19`. Line numbers in the digest and in the range are the same, so you and the model can both say "line 18".

Put the question into the same block with `-m`, and save it for later if you want:

```bash
cdg scan src/billing/ -l -m "Where is the rounding done?" -o ctx.xml
```

If the result is over 30,000 tokens or 100 files, `scan` asks first. Narrow it with paths, or use `cdg digest`.

## Leave out noise and secrets

Create a config with `cdg init`, then edit `.codigest/config.toml` to keep only the file types that matter and drop folders the model should never see:

```toml
[filter]
extensions = [".py", ".md"]
exclude_patterns = ["tests/data/", "*.lock"]
```

`.env` files and keys are never included whatever the config says. See [Configuration](/configuration) and [Security](/security).

## Capture a build and ask about it

```bash
cdg cap "npm run build | tail -20"
cdg term -f
```

`cdg term -f` copies the last command that failed. If you have installed the shell hook (`cdg term install`) you do not even need `cap`: run the build as usual, then `cdg pack` or `cdg term -f`.

## Let an agent do the gathering

A coding agent that can run commands needs no setup. Its shell is not a terminal, so Codigest prints for it and never touches the clipboard. A sequence that suits an agent:

```bash
cdg tree -L 2                      # where things are
cdg digest src/                    # what is defined where, with line numbers
cdg scan src/prices.py:16-19      # read the parts that matter, numbered
cdg mark                           # before editing
cdg semdiff                        # afterwards: which functions it changed
cdg cap -- pytest -q || cdg pack   # run the tests; on failure, the pack has what to fix
```

An agent that speaks MCP can ask for the same output as tools, without a shell. See [MCP Integration](/mcp).

## Switch projects without losing the failure

Captures are kept once per machine, not per project. A failure you captured in `~/work/api` is not offered when you run `cdg pack` in `~/work/web`: `pack` looks for failures run in the current project. `cdg term` shows the latest command anywhere, and `cdg term --here` limits it to the current folder and below.
