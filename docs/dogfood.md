# Using Codigest on itself

Notes from running `cdg` on real work in this repository and a few sibling projects, on Windows
(PowerShell and Git Bash). Each entry says what happened, what was wrong, and what changed.

## Lint errors across a whole package

`cdg cap -- uvx ruff check src`, then `cdg pack`.

- **Before:** ruff named 32 files. `pack` added 22 of them whole and dropped 10 for budget:
  about 30k tokens, most of it code far from any error.
- **Change:** a file that the output only points at, and that you did not change, now comes as
  an excerpt: ten lines around each reference, with a `lines="..."` attribute. All 33 files fit
  in about 26k tokens, each with the context around its errors.
- **Change:** the summary line now says `4 places in failing output (excerpt)` instead of
  listing every line number, which wrapped at 80 columns.

## A traceback into a large module

A real `AttributeError` raised at line 255 of the 286-line `core/termstore.py`.

- The pack was about 1.6k tokens: the traceback, and lines 245 to 265 of `termstore.py`. The
  failing line and the loop around it were in the excerpt.
- **Bug:** the command `render([object()])` showed as `render()` in the summary; the terminal
  library read `[object()]` as a style tag. Commands and paths are now escaped, in `cdg term` too.
- **Change:** the excerpted file's imports (`redact.py`, `tags.py`) were added whole and were not
  relevant. Imports are now followed only from files you named, changed, or tests of those.

## A passing run after an old failure

Tests passed, but `pack` still used a ruff failure from minutes earlier and filled the budget with
lint files.

- With the shell hook, every `ls` and `git status` is captured, so "the last command passed"
  cannot mean "nothing is failing". Failures older than an hour are now ignored.
- A failure from a few minutes ago is still used. The summary names it and its age on the first
  line, so it is visible, but this case stays a judgment call.

## `cdg cap -- npm test` on Windows

- **Bug:** `Command not found: npm`. npm on Windows is `npm.cmd`, which the process API does not
  find without the extension. Relative paths with forward slashes (`.venv/Scripts/python`) failed
  the same way from Git Bash. The program is now resolved the way a shell resolves it.
- **Bug:** npm's exit code `-4058` was stored as `4294963238`. Exit codes are now shown signed.

## Other projects

`cdg cap -- uv run --with pytest pytest -q` in deai, hanfix, QoraPhase and QoraGeom.

- Passing suites with a clean tree: `pack` says there is nothing to pack and what to run. Correct.
- hanfix exited 5 (pytest collected no tests); the pack was the 64-token output and nothing else.
  Correct, if not very useful.

## Where it helps, and where it doesn't

- **Pasting into a chat model.** The clearest win. One command replaces copying the error, the
  test and the changed file, and the `reason` on each file plus the `<omitted>` list tell the
  model what it has and lacks.
- **An agent that can read files itself** (Claude Code, Codex). The file contents save a few tool
  calls but are not essential. What the agent cannot get otherwise is the output of commands the
  person ran in their own terminal; that is the part of `pack` and `terminal_history` that matters
  here. When the agent runs the command itself, it already has the output.
- **Token counts** are estimates (four characters per token). They were within the budget in
  every run above, but they are not exact.
