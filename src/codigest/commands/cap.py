"""
`cdg cap` -- run a command transparently and record it for later recall.

Registered on the root app as a single command (not a Typer group), so the
first word after `cap` is treated as the wrapped program rather than a
subcommand. It declares no options of its own: everything after `cap` belongs
to the wrapped command, so flags like `-n` are never stolen from it.
"""
from pathlib import Path
from typing import Annotated

import typer
from rich.console import Console

lazy from ..core import runner, termstore, shellhist

err_console = Console(stderr=True)

CONTEXT_SETTINGS = {
    "allow_extra_args": True,
    "ignore_unknown_options": True,
}

CommandArgument = Annotated[
    list[str] | None,
    typer.Argument(
        metavar="COMMAND...",
        help="Command to run. Prefix with `--` so its flags are passed through.",
    ),
]


def handle(ctx: typer.Context, command: CommandArgument = None) -> None:
    """
    Run a command as usual and keep its output for `pack` and `term`.

    Examples:

        cdg cap -- pytest -q

        cdg cap "npm run build | tail -20"
    """
    args = [*(command or []), *ctx.args]

    if not args:
        err_console.print(
            "[yellow]Usage:[/yellow] cdg cap -- <command>\n"
            "[dim]Example: cdg cap -- pytest -q[/dim]"
        )
        raise typer.Exit(code=2)

    command_text = runner.join_command(args)

    try:
        exit_code, output, duration_ms = runner.run(args)
    except FileNotFoundError as e:
        err_console.print(f"[red][Error][/red] Command not found: {args[0]}")
        raise typer.Exit(code=127) from e
    except RuntimeError as e:
        err_console.print(f"[red][Error][/red] {e}")
        raise typer.Exit(code=1) from e

    entry = termstore.Entry(
        command=command_text,
        output=output,
        exit_code=exit_code,
        cwd=str(Path.cwd()),
        shell=shellhist.detect_shell(),
        started_at=termstore.now_iso(),
        duration_ms=duration_ms,
    )

    try:
        termstore.append_entry(entry)
    except OSError as e:
        err_console.print(f"[dim]⚠️ Capture not stored: {e}[/dim]")

    raise typer.Exit(code=exit_code)
