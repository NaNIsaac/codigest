# Command Reference

`cdg` is the short name for `codigest`. `cdg --help` lists the commands in these groups, and `cdg <command> --help` lists the options of one.

| Group | Commands |
| --- | --- |
| Share code | [`scan`](#cdg-scan), [`digest`](#cdg-digest), [`tree`](#cdg-tree) |
| Share changes | [`mark`](#cdg-mark), [`diff`](#cdg-diff), [`semdiff`](#cdg-semdiff) |
| Share a failing command | [`cap`](#cdg-cap), [`pack`](#cdg-pack), [`term`](#cdg-term) |
| Agents | [`mcp`](#cdg-mcp) |
| Setup | [`status`](#cdg-status), [`init`](#cdg-init), [`clean`](#cdg-clean), [`shell-help`](#cdg-shell-help) |

`cdg --version` (`-v`) prints the installed version.

## Output options {#output-options}

Most commands produce one block of text and share the same options for where it goes.

- In a terminal the block is copied to the clipboard. A status line says so.
- When the output goes to a pipe or a file (`cdg scan > ctx.xml`, `cdg digest | less`, a script, a coding agent's shell), it is printed, the clipboard is left alone, and status messages go to stderr. Add `--copy` to copy anyway.
- `-s` prints the block in a terminal as well. `-o FILE` also writes it to a file.
- Over SSH (`SSH_CONNECTION`, `SSH_CLIENT` or `SSH_TTY` is set) the block goes to the clipboard of the machine you type on, through the OSC 52 escape sequence, instead of the remote machine's clipboard. Your terminal has to accept OSC 52; inside tmux, add `set -g set-clipboard on`. `CODIGEST_CLIPBOARD=osc52` forces this (for example in `docker exec`) and `CODIGEST_CLIPBOARD=system` turns it off.

| Option | Meaning |
| --- | --- |
| `-c`, `--copy`, `--no-copy` | Force or prevent copying to the clipboard. |
| `-s`, `--stdout` | Also print the result. On by default when the output is piped. |
| `-o`, `--output` | Also write the result to this file. |
| `-m`, `--message` | Put your question or instruction in the output, as a `<user_instruction>` block. |
| `-r`, `--resolve` | Also pull in the local files imported by the targets, recursively. |
| `-a`, `--all` | Ignore the filters in `.codigest/config.toml`. |

Each command below lists the options it accepts. Not every command takes every shared option.

## Formats {#formats}

`scan` and `term` print XML blocks unless told otherwise. `--format markdown` is for chats that render Markdown: `scan` gives the instruction, the structure, and a heading and a fenced block per file with the language of its extension; `term` gives a heading per command, its details as a list, and the command with its output in one `console` block. A fence is always longer than any run of backticks inside it. `--format json` is for scripts. The block cache holds XML, so the other formats read the files afresh.

`scan --format json` is an object with `project`, `instruction`, `structure` and `files` (`path`, `lines` for a range, `content`). `term --format json` is an array with `index` counting back from the latest, `command`, `exit_code`, `cwd`, `started_at`, `duration_ms`, `output`, `output_available` and `truncated`, plus `hostname` and `shell` with `--context` and `note` when there is one.

```bash
cdg scan src/app.py:40-60 --format markdown -m "Why does this loop twice?"
cdg term -n 3 --failed --format json
```

## Presets {#presets}

A preset is a set of options saved under a name. `--preset NAME` works on every command: it puts the saved options right after the command name, so anything typed on the command line wins. It is expanded before the options are parsed and does not appear in `--help`.

```toml
# %APPDATA%\codigest\config.toml on Windows, ~/.config/codigest/config.toml elsewhere,
# or .codigest/config.toml in the project (replaces a user preset of the same name)
[presets.term.debug]
format = "markdown"
count = 3
failed = true
```

```bash
cdg term --preset debug       # last 3 failed commands as Markdown
cdg term --preset debug -n 1  # the same, only the last one
```

Keys are option names with `_` for `-` (`no_copy = true` is `--no-copy`). `true` adds a flag, `false` leaves it out, and lists are joined with commas. An unknown preset stops with exit code 2 and names the file to define it in. When an option is given twice, the later one wins.

## `cdg scan`

Bundle the whole project, or only the given paths, as one block.

```console
cdg scan [OPTIONS] [targets]...
```

Without targets it takes every file that is not ignored: the file tree and each file's contents, honoring `.gitignore`, the [config filters](/configuration) and the [always-skipped folders](/security#what-is-never-included).

A target is a file, a folder, or a file with a line range: `src/app.py:40-80` for lines 40 to 80, `src/app.py:42` for one line. A range is numbered and the `<file>` block gets a `lines` attribute. The same file may appear more than once with different ranges.

| Option | Meaning |
| --- | --- |
| `-o`, `--output` | Also write the result to this file. |
| `-c`, `--copy`, `--no-copy` | Copy to the clipboard (default: copy in a terminal, not when piped). |
| `-s`, `--stdout` | Also print the result. |
| `-a`, `--all` | Ignore the config filters. |
| `-m`, `--message` | Add an instruction to the output. |
| `-l`, `--lines` | Add line numbers to code blocks. |
| `-y`, `--yes` | Skip the size confirmation. |
| `-r`, `--resolve` | Recursively resolve imports of the targets. |
| `--format` | `xml` (default), `markdown` or `json`. See [Formats](#formats). |

Above 30,000 estimated tokens or 100 files, `scan` asks before continuing. When nobody can answer (no terminal on stdin), it stops with exit code 1 and a hint instead of waiting; `-y` takes everything. A line names any credential files that were withheld.

```bash
cdg scan                      # the whole project
cdg scan src/app.py -r        # one file and the local files it imports
cdg scan src/billing/ docs/   # folders, files, or both
cdg scan src/app.py -l        # with line numbers
cdg scan src/app.py:40-80     # only lines 40 to 80
```

## `cdg digest`

Outline Python and MATLAB files: numbered signatures and docstrings, no bodies.

```console
cdg digest [OPTIONS] [targets]...
```

Python files (`.py`, `.pyi`) list their functions, classes and top-level variables. MATLAB files (`.m`) list functions and `%%` sections. Each entry has the line it starts on and the first line of its docstring or help comment. Class attributes and dataclass fields are listed with their values, shortened when long. Other file types are left out of the digest, though they still appear in the tree.

| Option | Meaning |
| --- | --- |
| `-c`, `--copy`, `--no-copy` | Copy to the clipboard (default: copy in a terminal, not when piped). |
| `-o`, `--output` | Also write the result to this file. |
| `-s`, `--stdout` | Also print the result. |
| `-m`, `--message` | Add an instruction to the output. |
| `-r`, `--resolve` | Recursively resolve imports of the targets. |

```bash
cdg digest                # the whole project
cdg digest src/billing/   # one folder
```

## `cdg tree`

Print the project file tree.

```console
cdg tree [OPTIONS] [target]
```

In a terminal the tree is drawn with file sizes. With `-L`, `-s` or a pipe it is plain text, and that text is what `--copy` and `-o` receive. Unlike the other commands, `tree` does not copy unless you ask with `--copy`.

| Option | Meaning |
| --- | --- |
| `-c`, `--copy`, `--no-copy` | Copy to the clipboard (default: off). |
| `-o`, `--output` | Also write the result to this file. |
| `-s`, `--stdout` | Also print the result. |
| `-a`, `--all` | Ignore the config filters. |
| `-r`, `--resolve` | Recursively resolve imports. |
| `-L`, `--depth` | Show this many levels; deeper folders become a file count. Minimum 1. |

```console
$ cdg tree -L 1
├── .gitignore
├── legacy/ (1 file)
├── prices.py
└── test_prices.py
```

## `cdg mark`

Set the baseline that `diff`, `semdiff` and `pack` compare against.

```console
cdg mark [target]
```

The baseline is a private copy of your files kept in `.codigest/`. Your Git history is not touched. Running `mark` again moves the baseline. It takes no options.

```console
$ cdg mark
Baseline set. `cdg diff` now shows changes from here.
```

## `cdg diff`

Show the text diff since the baseline.

```console
cdg diff [OPTIONS] [target]
```

Exits with code 1 and asks you to run `cdg mark` when there is no baseline. Hunks of credential files are replaced with a notice and tokens are masked.

| Option | Meaning |
| --- | --- |
| `-c`, `--copy`, `--no-copy` | Copy to the clipboard (default: copy in a terminal, not when piped). |
| `-o`, `--output` | Also write the result to this file. |
| `-s`, `--stdout` | Also print the result. |
| `-m`, `--message` | Add an instruction to the output. |
| `-r`, `--resolve` | Recursively resolve imports. |

```console
$ cdg diff
Checking changes since last mark (1 second ago)...
Changes Detected! (877 chars)
<changes project="project" source="since cdg mark (1 second ago)">
diff --git a/prices.py b/prices.py
index 9c00411..34ef031 100644
--- a/prices.py
+++ b/prices.py
@@ -3,7 +3,7 @@ TAX_RATE = 0.1
 
 def apply_discount(price: float, percent: float) -> float:
     """Price after taking off `percent` percent."""
-    return price * (1 - percent)
+    return price * (1 - percent / 100)
 
 
 class Cart:
@@ -13,7 +13,11 @@ class Cart:
         self.items = []
         self.percent = percent
 
-    def total(self) -> float:
+    def total(self, tax: bool = True) -> float:
         """Sum after discount and tax."""
         subtotal = sum(apply_discount(p, self.percent) for p in self.items)
-        return subtotal * (1 + TAX_RATE)
+        return subtotal * (1 + TAX_RATE) if tax else subtotal
+
+
+def tax_of(amount: float) -> float:
+    return amount * TAX_RATE
</changes>
```

## `cdg semdiff`

Show which Python functions and classes changed since the baseline.

```console
cdg semdiff [OPTIONS] [target]
```

Only `.py` and `.pyi` files that changed are compared. Each line starts with a marker:

| Marker | Meaning |
| --- | --- |
| `[ADDED]` | A new function, class, method or variable. |
| `[REMOVED]` | It was deleted. |
| `[SIGNATURE]` | Its signature changed; both versions follow `::`. |
| `[LOGIC]` | The signature is the same but the body changed. |

A file that is new or deleted since the baseline is marked `(NEW)` or `(DELETED)` in its path.

| Option | Meaning |
| --- | --- |
| `-c`, `--copy`, `--no-copy` | Copy to the clipboard (default: copy in a terminal, not when piped). |
| `-o`, `--output` | Also write the result to this file. |
| `-s`, `--stdout` | Also print the result. |
| `-m`, `--message` | Add an instruction to the output. |
| `-r`, `--resolve` | Recursively resolve imports. |

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

## `cdg cap`

Run a command as usual and keep its output for `pack` and `term`.

```console
cdg cap [--] COMMAND...
```

`cap` has no options of its own: everything after it belongs to the wrapped command. Put `--` before the command so its flags are passed through. The output streams live, exactly as without `cap`, and stdout and stderr are recorded as one interleaved stream. `cap` exits with the wrapped command's exit code, so it can sit in front of a command in a script.

A single argument containing shell characters (`|`, `&`, `>`, `<`, `;`, `$`, `` ` ``, `*`, `?`, `(`, `)`) runs through a shell, which is how pipelines work: `cdg cap "npm run build | tail -20"`. That shell is PowerShell on Windows when installed, otherwise `$SHELL` or bash. Set `CODIGEST_SHELL` to use another one.

| Exit code | When |
| --- | --- |
| the command's own | The command ran. |
| 2 | No command was given. |
| 127 | The program was not found. |

Captures are stored once per machine, not per project; see [Terminal Capture](/terminal-capture).

```bash
cdg cap -- pytest -q
cdg cap "npm run build | tail -20"
```

## `cdg pack`

Bundle the last failure, the files it points at and your changes.

```console
cdg pack [OPTIONS] [targets]...
```

Plain `cdg pack` fills a token budget in a fixed order: the files named in the last failed command, the code that failing tests call, files you changed, tests that import those, and project files they import. Naming files packs only those files. [How `pack` decides](/getting-started#how-pack-decides) has the full order. Exits with code 1 when there is nothing to pack.

| Option | Meaning |
| --- | --- |
| `-m`, `--message` | Add an instruction to the output. |
| `-b`, `--budget` | Token budget, estimated at four characters per token. Default 30000, minimum 1000. |
| `--with` | Automatic sources to add: any of `failure`, `changes`, `tests`, `imports` (comma separated), or `none`. |
| `-c`, `--copy`, `--no-copy` | Copy to the clipboard (default: copy in a terminal, not when piped). |
| `-s`, `--stdout` | Also print the result. |
| `-o`, `--output` | Also write the result to this file. |

Without `--with`, all four sources are on; when you name files, none are. The failed command and the diff are always included when their source is on. The summary on stderr lists each file with the reason it was added and names what was left out.

```console
$ cdg pack prices.py --with tests
Packed ~209 / 30,000 tokens
  auto     tests (--with failure,changes,tests,imports adds more)
  + prices.py       requested
  + test_prices.py  tests prices.py
```

```bash
cdg pack                             # last failure here + your changes
cdg pack src/prices.py               # just this file
cdg pack src/prices.py --with tests  # this file and its tests
cdg pack --with changes              # only your changes, no failure
cdg pack -m "fix the discount bug"   # add an instruction
```

## `cdg term`

Show recently captured commands and their output.

```console
cdg term [OPTIONS] [COMMAND]
```

Without a subcommand it prints the most recent captures. When nothing was captured it falls back to the native shell history (PowerShell, bash, zsh, fish): the commands without their output.

| Option | Meaning |
| --- | --- |
| `-n`, `--count` | Number of recent commands to include. Default 1. |
| `-i`, `--index` | Pick exactly one command, counting back from the latest (1 = last). |
| `-f`, `--failed` | Only commands that exited non-zero. |
| `--here` | Only commands run in the current directory or below. |
| `--history-only` | Read the native shell history instead of Codigest captures (no output). |
| `--context` | Include the captured hostname and shell in the output. |
| `--format` | `xml` (default), `markdown` or `json`. See [Formats](#formats). |
| `-c`, `--copy`, `--no-copy` | Copy to the clipboard (default: copy in a terminal, not when piped). |
| `-s`, `--stdout` | Also print the result. |
| `-o`, `--output` | Also write the result to this file. |

```bash
cdg term              # last command and its output
cdg term -n 3         # last 3
cdg term -f           # last failed one
cdg term --here -n 3  # last 3 run in this project
cdg term -i 2         # only the command from 2 runs ago
cdg term --context    # add hostname and shell
```

#### `--raw`, only in the binary {#term-raw}

The prebuilt binary (the Rust build) has one more option here: `term --raw` prints the stored output of one command without tags. The Python package does not have it. See [Rust Port](/rust-port).

### `cdg term install`

Hook your shells so every command's output is captured automatically.

| Option | Meaning |
| --- | --- |
| `--shell` | `powershell`, `bash`, `zsh` or `all`. Default: every one installed. |

### `cdg term uninstall`

Remove the capture hook from your shell startup files.

| Option | Meaning |
| --- | --- |
| `--shell` | `powershell`, `bash`, `zsh` or `all`. Default: every one installed. |

### `cdg term clear`

Delete all stored terminal captures. It takes no options.

### `cdg term path`

Show where terminal captures are stored. It takes no options.

### `cdg term ingest`

Store one entry. The shell hook calls this; it is hidden from `--help` and not meant to be run by hand. PowerShell passes JSON on stdin. bash and zsh pass a temporary file and these options.

| Option | Meaning |
| --- | --- |
| `--file` | A file holding the command, a NUL byte, then the output. Deleted afterwards. |
| `--exit-code` | The command's exit code. |
| `--cwd` | The directory it ran in. |
| `--shell` | The shell it ran in. |
| `--start` | Start time, in epoch seconds. |
| `--end` | End time, in epoch seconds. |

If an ingest fails, the error is appended to `ingest-errors.log` next to the captures, and `cdg term` mentions it.

## `cdg mcp`

Serve these tools to AI agents over MCP (stdio).

```console
cdg mcp
```

It takes no options and speaks newline-delimited JSON-RPC on stdin and stdout. See [MCP Integration](/mcp) for registering it and for the tools it offers.

## `cdg status`

Show what `pack` would use here: the last failure, your changes and the baseline.

```console
cdg status [target]
```

It takes no options and writes nothing.

```console
$ cdg status
project  ~/project
  failure   pytest -q (exit 1, just now)
  changes   1 file, since cdg mark (2 seconds ago)
            prices.py
  baseline  set 2 seconds ago (cdg mark moves it)
  files     4 considered
```

## `cdg init`

Create `.codigest/config.toml` (file filters).

```console
cdg init [OPTIONS] [target]
```

Writes the config file, makes `.codigest/` invisible to Git with its own `.gitignore`, and adds `.codigest/` to the project's `.gitignore`. It does not set a baseline; run `cdg mark` for that. You never need `init` for the other commands to work.

| Option | Meaning |
| --- | --- |
| `-f`, `--force` | Overwrite an existing config. |

## `cdg clean`

Delete `.codigest/`: the config, the baseline and the cache.

```console
cdg clean [OPTIONS] [target]
```

Asks first unless told otherwise. Your source files are not touched.

| Option | Meaning |
| --- | --- |
| `-y`, `--yes`, `-f`, `--force` | Skip the confirmation prompt. |

## `cdg shell-help`

Let PowerShell `Get-Help` find codigest without enabling capture.

PowerShell's `help codigest` is a separate command from `codigest --help`. `shell-help install` adds a small forwarding function to your PowerShell profile, and `uninstall` removes it. Other `help` commands are unchanged, and no command capture is turned on. Open a new PowerShell window afterwards. Both subcommands take no options.

### `cdg shell-help install`

Register PowerShell help for codigest (opt-in).

### `cdg shell-help uninstall`

Remove only the Codigest PowerShell help integration.
