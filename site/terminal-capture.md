# Terminal Capture

`cdg cap` streams the command's output live and records the command line, combined stdout and stderr, exit code and working directory. `cdg term` hands the records back, and `cdg pack` uses the last failed one.

Captures from all projects share one store on your machine, so `cap` and `term` work in any directory, with or without a repository.

## Capture one command: cap

```bash
cdg cap -- pytest -q                # flags after -- belong to the command
cdg cap "npm run build | tail -20"  # quote pipelines
```

The output looks exactly as it would without `cap`. The exit code is passed through. See [`cdg cap`](/commands#cdg-cap) for how pipelines are run.

## Read captures back: term

```bash
cdg term              # last command and its output
cdg term -n 3         # last 3
cdg term -f           # last failed one
cdg term --here -n 3  # last 3 run in this project
cdg term -i 2         # only the command from 2 runs ago
cdg term --context    # add the PC hostname and shell to the copied context (opt-in)
cdg term path         # where captures are stored (override with CODIGEST_HOME)
cdg term clear        # delete all captures
```

Commands not run through `cap` are read from the native shell history (PowerShell, bash, zsh, fish), without their output. `--history-only` reads only that history.

The hostname and shell name are left out by default because a PC name may be sensitive. `--context` adds `hostname` and `shell` attributes to the copied block. The hostname is recorded when the command is captured, so older captures have none and `cdg term` still works with them.

## Capture everything: install

`cdg term install` edits your PowerShell profile, `~/.bashrc` and `~/.zshrc` for whichever of these shells is installed. Choose with `--shell powershell|bash|zsh|all`. On Windows, Git Bash counts as bash. `cdg term uninstall` removes the hook.

Commands still run in your current session: `cd`, variables and functions work normally, and the command appears in your history as typed. Editors, pagers, REPLs and `ssh` are skipped.

```bash
cdg term install                 # every shell found
cdg term install --shell bash    # one shell
cdg term uninstall
```

The change is a marked block in each startup file, between `# >>> codigest terminal capture >>>` and `# <<< codigest terminal capture <<<`. It is active in new shell windows. The install message prints the line that enables it in the current one.

### Controls

- Skip one command: start the line with a space.
- Skip more programs: `$env:CODIGEST_CAP_SKIP = '^\s*(myrepl|mytui)\b'` in PowerShell, or `export CODIGEST_CAP_SKIP='^(myrepl|mytui)( |$)'` (an extended regex) in bash and zsh.
- Disable for a session: `$env:CODIGEST_NOCAP = 1` in PowerShell, `export CODIGEST_NOCAP=1` in bash and zsh.
- Remove the hooks: `cdg term uninstall`.

`cdg term` tells you when the hook is not active in the current shell and how to turn it on for that window.

### How the hooks work

**PowerShell.** The hook replaces `PSConsoleHostReadLine`. After PSReadLine returns the line you typed, the hook hands the engine a wrapped form that runs your line in the caller's scope and tees its output. The screen and the history keep the line as typed.

**bash and zsh.** Enter records the line. Before it runs, the shell's stdout and stderr go through `tee` into a private temporary file; before the next prompt they are restored.

In both cases the finished record is handed to `cdg term ingest` in a background process, and redaction happens there, before anything reaches disk. A failed ingest is logged to `ingest-errors.log` next to the captures.

### Limitations

- Captured programs write to a pipe, not a TTY, so some lose colors or progress bars.
- In bash (4+) and zsh, multi-line commands (loops typed over several lines, heredocs), background jobs (`cmd &`) and lines using `!` history expansion are not captured; they run normally. In bash, neither are lines that start with a subshell `( ... )`.
- In bash the hook uses the DEBUG trap and keeps one set before the first prompt (starship, bash-preexec); a DEBUG trap set later replaces it.

## What is stored

Each record holds the command, its output, exit code, working directory, shell, hostname, start time and duration.

- **Cleaned.** Color codes are stripped, and progress-bar redraws (carriage returns) keep only their final state, so one `pytest` or `pip` run does not store megabytes of frames.
- **Masked.** Tokens and `password=...` style values are replaced before the record is written. Output of credential commands is not stored at all. See [Security](/security#in-terminal-captures).
- **Capped.** One newline-delimited JSON file keeps the last 200 commands. Each output is capped at 64 KB, keeping the first and last 32 KB with a marker between; such a record has `truncated="true"` in the block.
- **Where.** `cdg term path` shows the file. On macOS and Linux it is readable only by you. Set `CODIGEST_HOME` to move it; see [Architecture](/architecture#where-things-are-stored).
