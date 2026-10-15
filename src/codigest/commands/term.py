"""
`cdg term` -- recall recent terminal commands (and their output) as LLM context.
"""
import sys
from pathlib import Path
from typing import Annotated

import typer
from rich.console import Console

from ..options import CopyOption, StdoutOption

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
            case None: status = ""
            case 0: status = " [green]✓[/green]"
            case code: status = f" [red](exit {code})[/red]"
        marker = "" if entry.captured else " [yellow](no output)[/yellow]"
        log.print(f"[dim]{index}.[/dim] [cyan]{entry.command}[/cyan]{status}{marker}")


@app.callback(invoke_without_command=True)
def handle(
    ctx: typer.Context,
    count: CountOption = 1,
    index: IndexOption = None,
    failed: FailedOption = False,
    history_only: HistoryOnlyOption = False,
    copy: CopyOption = True,
    stdout: StdoutOption = False,
) -> None:
    """
    [Recall] Pull recent terminal commands and their captured output.

    Examples:
        cdg term            # the last command
        cdg term -n 3       # the last 3 commands
        cdg term -i 3       # only the command from 3 runs ago
        cdg term -f         # the last failed command
    """
    if ctx.invoked_subcommand:
        return

    handler = OutputHandler(stdout=stdout, copy=copy)
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
            log.print(
                "[dim]Capture output with `cdg term install` (PowerShell) "
                "or `cdg cap -- <command>`.[/dim]"
            )
        else:
            log.print("[yellow]No terminal history available.[/yellow]")
            log.print("[dim]Try: cdg term install, or cdg cap -- <command>[/dim]")
            raise typer.Exit(code=1)

    if failed:
        entries = [e for e in entries if e.exit_code not in (None, 0)]
        if not entries:
            log.print("[yellow]No failed commands captured.[/yellow]")
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

    content = termstore.render(selected)
    handler.handle_output(
        content,
        save_action=lambda: _summarize(handler, selected),
        item_name=f"{len(selected)} terminal entr{'y' if len(selected) == 1 else 'ies'}",
    )


@app.command("clear")
def clear() -> None:
    """Delete all stored terminal captures."""
    console = Console()
    removed = termstore.clear()
    console.print(f"[green]Cleared[/green] {removed} captured entries.")


@app.command("path")
def path() -> None:
    """Show where terminal captures are stored."""
    console = Console()
    console.print(str(termstore.get_history_path()))


@app.command("ingest", hidden=True)
def ingest() -> None:
    """Store one JSON entry from stdin (called by the shell hook)."""
    data = json.loads(sys.stdin.buffer.read().decode("utf-8-sig"))
    termstore.append_entry(termstore.Entry.from_dict(data))


# --------------------------------------------------------------------------
# PowerShell hook install
# --------------------------------------------------------------------------

HOOK_PATH = Path(__file__).resolve().parent.parent / "shell" / "hook.ps1"
_BEGIN = "# >>> codigest terminal capture >>>"
_END = "# <<< codigest terminal capture <<<"


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


def _strip_block(text: str) -> str:
    start = text.find(_BEGIN)
    end = text.find(_END)
    if start == -1 or end == -1:
        return text
    rest = (text[:start] + text[end + len(_END):]).strip("\n")
    return rest + "\n" if rest else ""


def _rewrite_profiles(add: bool) -> None:
    console = Console()
    if not (profiles := _powershell_profiles()):
        console.print("[red][Error][/red] No PowerShell found (pwsh / powershell).")
        raise typer.Exit(code=1)

    block = f"{_BEGIN}\nif (Test-Path '{HOOK_PATH}') {{ . '{HOOK_PATH}' }}\n{_END}\n"
    for profile in profiles:
        text = ""
        if profile.exists():
            try:
                text = profile.read_bytes().decode("utf-8-sig")
            except UnicodeDecodeError:
                # Rewriting a non-UTF-8 profile would corrupt it.
                console.print(f"[yellow]Skipped[/yellow] {profile} (not UTF-8; edit it by hand)")
                continue
        new = _strip_block(text)
        if add:
            new = (new.rstrip("\n") + "\n\n" if new.strip() else "") + block
        elif new == text:
            continue
        profile.parent.mkdir(parents=True, exist_ok=True)
        # BOM so Windows PowerShell 5.1 reads non-ASCII paths correctly.
        profile.write_bytes(new.encode("utf-8-sig"))
        console.print(f"[green]{'Installed' if add else 'Removed'}[/green] {profile}")

    if add:
        console.print("Active in new PowerShell windows. Enable in this window now with:")
        console.print(f"  . '{HOOK_PATH}'", highlight=False)
        console.print("[dim]Skip one command: start it with a space. Disable: $env:CODIGEST_NOCAP = 1[/dim]")


@app.command("install")
def install() -> None:
    """Hook PowerShell so every command's output is captured automatically."""
    _rewrite_profiles(add=True)


@app.command("uninstall")
def uninstall() -> None:
    """Remove the PowerShell capture hook from your profile."""
    _rewrite_profiles(add=False)
