"""
`cdg term` -- recall recent terminal commands (and their output) as LLM context.
"""

import codecs
import os
import sys
import traceback
from contextlib import suppress
from datetime import datetime
from pathlib import Path
from typing import Annotated

import typer
from rich.console import Console
from rich.markup import escape

from ..core.formats import Format
from ..options import CopyOption, FormatOption, OutputOption, StdoutOption

lazy import json
lazy import shutil
lazy import subprocess
lazy from ..core import termstore, shellhist
lazy from ..core.output import OutputHandler

app = typer.Typer()

CountOption = Annotated[
    int,
    typer.Option("-n", "--count", min=1, help="Number of recent commands to include"),
]

IndexOption = Annotated[
    int | None,
    typer.Option(
        "-i",
        "--index",
        min=1,
        help="Pick exactly one command, counting back from the latest (1 = last)",
    ),
]

FailedOption = Annotated[
    bool,
    typer.Option("-f", "--failed", help="Only commands that exited non-zero"),
]

HereOption = Annotated[
    bool,
    typer.Option("--here", help="Only commands run in the current directory or below"),
]

ContextOption = Annotated[
    bool,
    typer.Option("--context", help="Include captured PC hostname and shell in the copied output"),
]

HistoryOnlyOption = Annotated[
    bool,
    typer.Option(
        "--history-only",
        help="Read the native shell history instead of Codigest captures (no output)",
    ),
]


def _summarize(handler: OutputHandler, entries: list[termstore.Entry]) -> None:
    log = handler.log_console
    total = len(entries)
    for offset, entry in enumerate(entries):
        index = total - offset
        match entry.exit_code:
            case None:
                status = ""
            case 0:
                status = " [green]✓[/green]"
            case code:
                status = f" [red](exit {code})[/red]"
        marker = "" if entry.captured else " [yellow](no output)[/yellow]"
        log.print(f"[dim]{index}.[/dim] [cyan]{escape(entry.command)}[/cyan]{status}{marker}")


@app.callback(invoke_without_command=True)
def handle(
    ctx: typer.Context,
    count: CountOption = 1,
    index: IndexOption = None,
    failed: FailedOption = False,
    here: HereOption = False,
    history_only: HistoryOnlyOption = False,
    context: ContextOption = False,
    format: FormatOption = Format.XML,
    copy: CopyOption = None,
    stdout: StdoutOption = False,
    output: OutputOption = None,
) -> None:
    """
    Show recently captured commands and their output.

    Examples:

    \b
        cdg term              # the last command
        cdg term -n 3         # the last 3 commands
        cdg term -i 3         # only the command from 3 runs ago
        cdg term -f           # the last failed command
        cdg term --context    # include host + shell when copying for AI
        cdg term --here -n 3  # the last 3 commands run in this project
        cdg term --format json  # for scripts; markdown for a chat
    """
    if ctx.invoked_subcommand:
        return

    handler = OutputHandler(stdout=stdout, copy=copy, output=output)
    log = handler.log_console

    # -i N needs N entries loaded so the Nth-from-last exists.
    depth = index or count

    entries: list[termstore.Entry] = []
    if not history_only:
        entries = termstore.load_entries()

    if not entries:
        entries, shell = shellhist.read_recent(depth)
        if entries:
            log.print(
                f"[yellow][Warning][/yellow] No Codigest captures found; "
                f"falling back to {shell} history [dim](commands only, no output)[/dim]"
            )
            _diagnose(log)
        else:
            log.print("[yellow]No terminal history available.[/yellow]")
            _diagnose(log)
            raise typer.Exit(code=1)

    if failed or here:
        entries = termstore.select(entries, failed=failed, under=Path.cwd() if here else None)
        if not entries:
            what = " and ".join(
                w for w, on in (("failed", failed), ("in this directory", here)) if on
            )
            log.print(f"[yellow]No captured commands match ({what}).[/yellow]")
            raise typer.Exit(code=1)

    if index:
        if index > len(entries):
            log.print(
                f"[red][Error][/red] Only {len(entries)} entries available; "
                f"cannot reach index {index}."
            )
            raise typer.Exit(code=1)
        selected = [entries[-index]]
    else:
        selected = entries[-count:]

    render = {
        Format.XML: termstore.render,
        Format.MARKDOWN: termstore.render_markdown,
        Format.JSON: termstore.render_json,
    }[format]
    content = render(selected, include_context=context)
    handler.handle_output(
        content,
        summary=lambda: _summarize(handler, selected),
        item_name=f"{len(selected)} terminal entr{'y' if len(selected) == 1 else 'ies'}",
    )


@app.command("clear")
def clear() -> None:
    """Delete all stored terminal captures."""
    console = Console()
    removed = termstore.clear()
    (termstore.get_term_dir() / "ingest-errors.log").unlink(missing_ok=True)
    console.print(f"[green]Cleared[/green] {removed} captured entries.")


@app.command("path")
def path() -> None:
    """Show where terminal captures are stored."""
    console = Console()
    console.print(str(termstore.get_history_path()))


@app.command("ingest", hidden=True)
def ingest(
    file: Annotated[
        Path | None, typer.Option(help="Command, NUL, output; deleted afterwards")
    ] = None,
    exit_code: Annotated[int | None, typer.Option()] = None,
    cwd: Annotated[str, typer.Option()] = "",
    shell: Annotated[str, typer.Option()] = "",
    start: Annotated[str, typer.Option(help="Epoch seconds")] = "",
    end: Annotated[str, typer.Option(help="Epoch seconds")] = "",
) -> None:
    """
    Store one entry (called by the shell hook): JSON on stdin (PowerShell),
    or options plus a capture file (bash / zsh).
    """
    # The command travels in the file, not argv: click expands $VARS in argv on Windows.
    try:
        if file is None:
            data = json.loads(sys.stdin.buffer.read().decode("utf-8-sig"))
        else:
            command, _, output = _read_capture(file).partition(b"\0")
            # Locales with a decimal comma format $EPOCHREALTIME as 12,5.
            t0, t1 = (float(t.replace(",", ".")) for t in (start or "0", end or start or "0"))
            data = {
                "command": command.decode("utf-8", "replace"),
                "output": output.decode("utf-8", "replace"),
                "exit_code": exit_code,
                "cwd": str(Path(cwd)) if cwd else "",  # Git Bash passes D:/x
                "shell": shell,
                "started_at": datetime.fromtimestamp(t0).astimezone().isoformat(timespec="seconds"),
                "duration_ms": max(0, round((t1 - t0) * 1000)),
            }
        termstore.append_entry(termstore.Entry.from_dict(data))
    except Exception:
        # The hook discards our stderr; leave a trail `cdg term` can surface.
        with _ingest_log().open("a", encoding="utf-8") as f:
            f.write(f"--- {termstore.now_iso()}\n{traceback.format_exc()}")
        raise typer.Exit(code=1) from None
    finally:
        if file:
            file.unlink(missing_ok=True)


# Raw bytes read from each end of a capture file. The bash hook writes everything a command
# prints, so a server left running can leave a huge file; the store keeps 64 KB of it after
# cleaning up, and this leaves that cleanup plenty to work with.
RAW_KEEP = 1024 * 1024


def _read_capture(file: Path) -> bytes:
    size = file.stat().st_size
    with file.open("rb") as f:
        if size <= 4 * RAW_KEEP:
            return f.read()
        head = f.read(RAW_KEEP)
        f.seek(-RAW_KEEP, 2)
        marker = f"\n\n... [{size - 2 * RAW_KEEP} bytes elided by codigest] ...\n\n".encode()
        return head + marker + f.read()


def _ingest_log() -> Path:
    termstore.get_term_dir().mkdir(parents=True, exist_ok=True)
    return termstore.get_term_dir() / "ingest-errors.log"


def _diagnose(log) -> None:
    """Explains why nothing was captured instead of failing silently."""
    if not os.environ.get("CODIGEST_HOOK"):
        log.print(
            "[dim]Capture hook is not active in this shell. Run `cdg term install` "
            "and open a new window, or enable it now with:[/dim]"
        )
        log.print(f"[dim]  PowerShell: . '{HOOK_PATH}'[/dim]", highlight=False)
        log.print(f"[dim]  bash / zsh: . '{_posix(SH_HOOK_PATH)}'[/dim]", highlight=False)
    if (errors := termstore.get_term_dir() / "ingest-errors.log").exists():
        log.print(f"[red]Capture errors were logged:[/red] {errors}", highlight=False)


# --------------------------------------------------------------------------
# Hook install (PowerShell profiles, ~/.bashrc, ~/.zshrc)
# --------------------------------------------------------------------------

HOOK_PATH = Path(__file__).resolve().parent.parent / "shell" / "hook.ps1"
SH_HOOK_PATH = HOOK_PATH.with_name("hook.sh")
_BEGIN = "# >>> codigest terminal capture >>>"
_END = "# <<< codigest terminal capture <<<"
_SHELLS = ("auto", "powershell", "bash", "zsh", "all")

ShellOption = Annotated[
    str,
    typer.Option(
        "--shell",
        help="powershell, bash, zsh or all (default: every one installed)",
    ),
]


def _posix(path: Path) -> str:
    """C:/x/y -> /c/x/y, the form Git Bash understands."""
    if path.drive.endswith(":"):
        return f"/{path.drive[0].lower()}{path.as_posix()[2:]}"
    return path.as_posix()


def _powershell_profiles() -> list[Path]:
    """Asks each installed PowerShell for its CurrentUserCurrentHost profile path."""
    profiles = []
    for exe in ("pwsh", "powershell"):
        if not (found := shutil.which(exe)):
            continue
        result = subprocess.run(
            [found, "-NoProfile", "-Command", "$PROFILE.CurrentUserCurrentHost"],
            capture_output=True,
            text=True,
        )
        path = result.stdout.strip()
        if result.returncode == 0 and path:
            profiles.append(Path(path))
    return profiles


def _targets(shell: str) -> list[Path]:
    """Startup files for the requested shells; `auto` picks the installed ones."""
    if shell not in _SHELLS:
        raise typer.BadParameter(f"choose one of: {', '.join(_SHELLS)}", param_hint="--shell")
    wants = lambda name: shell in (name, "all") or (shell == "auto" and shutil.which(name))
    targets = []
    if shell in ("powershell", "all", "auto"):
        targets += _powershell_profiles()
    if wants("bash"):
        targets.append(Path.home() / ".bashrc")
    if wants("zsh"):
        targets.append(Path(os.environ.get("ZDOTDIR") or Path.home()) / ".zshrc")
    return targets


def _strip_block(text: str) -> str:
    """Removes the block and the one line break that install put in front of it."""
    start, end = text.find(_BEGIN), text.find(_END)
    if start == -1 or end < start:
        return text
    head, tail = text[:start], text[end + len(_END) :]
    head = head.removesuffix("\r\n") if head.endswith("\r\n") else head.removesuffix("\n")
    tail = tail.removeprefix("\r\n") if tail.startswith("\r\n") else tail.removeprefix("\n")
    return head + tail


def _with_block(text: str, line: str) -> str:
    """Appends the block after the text as it is, so `_strip_block` gives the text back."""
    rest = _strip_block(text)
    nl = "\r\n" if "\r\n" in rest else "\n"
    block = nl.join((_BEGIN, line, _END)) + nl
    return f"{rest}{nl}{block}" if rest else block


# What install found at each startup file (whether it existed, its BOM, folders it had
# to create), so uninstall can put back exactly that.
_STATE = "profiles.json"


def _load_state(path: Path) -> dict[str, dict]:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except OSError, ValueError:
        return {}


def _raw_capture_dirs() -> list[Path]:
    """The private folders where the bash / zsh hook keeps output until ingest reads it."""
    if os.name == "nt":
        # Git Bash's TMPDIR is %TEMP%, and its uid is not the Windows one Python sees.
        temp = os.environ.get("TEMP")
        return sorted(Path(temp).glob("codigest-[0-9]*")) if temp else []
    return [Path(os.environ.get("TMPDIR") or "/tmp") / f"codigest-{os.getuid()}"]


def _remove_raw_captures(console: Console) -> None:
    """
    Deletes capture files a day old or more (raw output whose ingest never ran) and the
    folder once it is empty. Newer files may belong to a command still running in a shell
    that loaded the hook before uninstall.
    """
    day_ago = datetime.now().timestamp() - 86400
    for folder in _raw_capture_dirs():
        if not folder.is_dir() or folder.is_symlink():
            continue
        for file in folder.iterdir():
            if file.is_file() and file.stat().st_mtime < day_ago:
                file.unlink(missing_ok=True)
        try:
            folder.rmdir()
        except OSError:
            console.print(f"[yellow]Kept[/yellow] {folder} (a hooked shell may still write to it)")


def _rewrite_profiles(add: bool, shell: str) -> None:
    console = Console()
    # Removing from a file that has no block is a no-op, so uninstall checks every shell.
    everywhere = not add and shell in ("auto", "all")
    if not (targets := _targets("all" if everywhere else shell)):
        console.print("[red][Error][/red] No supported shell found (PowerShell, bash, zsh).")
        raise typer.Exit(code=1)

    data_dir = termstore.get_data_dir()
    shell_dir = data_dir / "shell"
    state_path = shell_dir / _STATE
    state = _load_state(state_path)
    hook_ps1, hook_sh = HOOK_PATH, SH_HOOK_PATH
    if add:
        # Profiles load a copy, not the package itself: with an editable install the package
        # is a working tree, and whatever state it is in would run in every new shell.
        # `cdg term install` again refreshes the copy.
        shell_dir.mkdir(parents=True, exist_ok=True)
        hook_ps1, hook_sh = shell_dir / HOOK_PATH.name, shell_dir / SH_HOOK_PATH.name
        shutil.copyfile(HOOK_PATH, hook_ps1)
        shutil.copyfile(SH_HOOK_PATH, hook_sh)
    sh = _posix(hook_sh)
    for target in targets:
        is_ps = target.suffix == ".ps1"
        raw = target.read_bytes() if target.is_file() else b""
        try:
            text = raw.decode("utf-8-sig")
        except UnicodeDecodeError:
            # Rewriting a non-UTF-8 profile would corrupt it.
            console.print(f"[yellow]Skipped[/yellow] {target} (not UTF-8; edit it by hand)")
            continue
        if add:
            line = (
                f"if (Test-Path '{hook_ps1}') {{ . '{hook_ps1}' }}"
                if is_ps
                else f"if [ -f '{sh}' ]; then . '{sh}'; fi"
            )
            new = _with_block(text, line)
            state.setdefault(
                str(target),
                {
                    "existed": target.is_file(),
                    "bom": raw.startswith(codecs.BOM_UTF8),
                    "dirs": [str(p) for p in target.parents if not p.exists()],
                },
            )
            # BOM so Windows PowerShell 5.1 reads non-ASCII paths correctly; it would break bash.
            bom = is_ps
        else:
            before = state.pop(str(target), None)
            if (new := _strip_block(text)) == text:
                continue
            if before and not before["existed"] and not new:
                target.unlink()
                for folder in before["dirs"]:
                    with suppress(OSError):
                        Path(folder).rmdir()
                console.print(f"[green]Removed[/green] {target}")
                continue
            # A block from an install that kept no record: leave the BOM as it is.
            bom = before["bom"] if before else raw.startswith(codecs.BOM_UTF8)
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes((codecs.BOM_UTF8 if bom else b"") + new.encode())
        console.print(f"[green]{'Installed' if add else 'Removed'}[/green] {target}")

    if add:
        state_path.write_text(json.dumps(state, indent=1), encoding="utf-8")
        console.print("Active in new shell windows. Enable in this one now with:")
        if any(t.suffix == ".ps1" for t in targets):
            console.print(f"  PowerShell:  . '{hook_ps1}'", highlight=False)
        if any(t.suffix != ".ps1" for t in targets):
            console.print(f"  bash / zsh:  . '{sh}'", highlight=False)
        console.print(
            "[dim]Skip one command: start it with a space. Disable: set CODIGEST_NOCAP=1[/dim]"
        )
    elif everywhere:
        for name in (HOOK_PATH.name, SH_HOOK_PATH.name, _STATE):
            (shell_dir / name).unlink(missing_ok=True)
        for folder in (shell_dir, data_dir):
            with suppress(OSError):
                folder.rmdir()
        _remove_raw_captures(console)
        if termstore.get_history_path().exists():
            console.print(
                f"[dim]Captured commands are kept in {termstore.get_history_path()}; "
                "`cdg term clear` deletes them.[/dim]",
                highlight=False,
            )
    elif state_path.exists():
        state_path.write_text(json.dumps(state, indent=1), encoding="utf-8")


@app.command("install")
def install(shell: ShellOption = "auto") -> None:
    """Hook your shells so every command's output is captured automatically."""
    _rewrite_profiles(add=True, shell=shell)


@app.command("uninstall")
def uninstall(shell: ShellOption = "auto") -> None:
    """Remove the capture hook from your shell startup files."""
    _rewrite_profiles(add=False, shell=shell)
