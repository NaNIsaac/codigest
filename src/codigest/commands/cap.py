"""
`cdg cap` -- run a command transparently and record it for later recall.

Registered on the root app as a single command (not a Typer group), so the
first word after `cap` is treated as the wrapped program rather than a
subcommand. Everything after `cap` belongs to the wrapped command, so flags
like `-n` are never stolen from it. Only output options written before the
command (`cdg cap -c -- pytest`) are read, by hand, and they make `cap` hand
over the result like `cdg term` would.
"""

import base64
import shutil
from dataclasses import dataclass
from pathlib import Path
from typing import Annotated

import typer
from rich.console import Console

from ..core.formats import Format
lazy from ..core import runner, termstore, shellhist
lazy from ..core.output import OutputHandler

err_console = Console(stderr=True)

CONTEXT_SETTINGS = {
    "allow_extra_args": True,
    "ignore_unknown_options": True,
}

CommandArgument = Annotated[
    list[str] | None,
    typer.Argument(
        metavar="COMMAND...",
        help=(
            "Command to run. Prefix with `--` so its flags are passed through. Output options "
            "before it (-c, -s, -o FILE, --format) also hand over the result like `cdg term`."
        ),
    ),
]


@dataclass(slots=True)
class Delivery:
    """What `cap` does with the result once the command has run."""

    copy: bool | None = None
    stdout: bool = False
    output: Path | None = None
    format: Format = Format.XML
    requested: bool = False


def split_options(args: list[str]) -> tuple[Delivery, list[str]]:
    """Reads the output options in front of the command. Parsed by hand because the
    option parser would also pick them out of the wrapped command's own arguments."""
    delivery = Delivery()
    i = 0
    while i < len(args):
        name, eq, inline = args[i].partition("=")
        if not (eq and name.startswith("--")):
            name, inline = args[i], None
        takes_value = name in ("-o", "--output", "--format")
        if inline is not None and not takes_value:
            break
        match name:
            case "-c" | "--copy":
                delivery.copy = True
            case "--no-copy":
                delivery.copy = False
            case "-s" | "--stdout":
                delivery.stdout = True
            case "--":
                return delivery, args[i + 1 :]
            case "-o" | "--output" | "--format":
                if inline is None:
                    i += 1
                    if i == len(args):
                        raise ValueError(f"{name} needs a value")
                    inline = args[i]
                if name == "--format":
                    try:
                        delivery.format = Format(inline.lower())
                    except ValueError:
                        raise ValueError(
                            f"unknown format '{inline}' (xml, markdown or json)"
                        ) from None
                else:
                    delivery.output = Path(inline)
            case _:
                break
        delivery.requested = True
        i += 1
    return delivery, args[i:]


def script_block(args: list[str]) -> str | None:
    """PowerShell hands a `{ ... }` argument to a program as `-encodedCommand <base64 of
    UTF-16LE>` followed by `-inputFormat xml -outputFormat text`, the form `pwsh { ... }`
    receives. Returns the script text."""
    if len(args) < 2 or args[0].lower() != "-encodedcommand":
        return None
    try:
        return base64.b64decode(args[1], validate=True).decode("utf-16-le")
    except ValueError:
        return None


def handle(ctx: typer.Context, command: CommandArgument = None) -> None:
    """
    Run a command as usual and keep its output for `pack` and `term`.

    Examples:

        cdg cap -- pytest -q

        cdg cap "npm run build | tail -20"

        cdg cap --copy { Get-ChildItem | Sort-Object Length }
    """
    try:
        delivery, args = split_options([*(command or []), *ctx.args])
    except ValueError as e:
        err_console.print(f"[red][Error][/red] {e}")
        raise typer.Exit(code=2) from e

    if not args:
        err_console.print(
            "[yellow]Usage:[/yellow] cdg cap -- <command>\n[dim]Example: cdg cap -- pytest -q[/dim]"
        )
        raise typer.Exit(code=2)

    try:
        if (script := script_block(args)) is not None:
            shell = shutil.which("pwsh") or shutil.which("powershell")
            if not shell:
                err_console.print("[red][Error][/red] PowerShell is needed to run a { } block")
                raise typer.Exit(code=127)
            command_text = script.strip()
            exit_code, output, duration_ms = runner.run_argv(
                [shell, "-NoProfile", "-EncodedCommand", args[1]]
            )
        else:
            command_text = runner.join_command(args)
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
        # Normalizes, redacts and truncates `entry` in place: what is delivered is what is stored.
        termstore.append_entry(entry)
    except OSError as e:
        err_console.print(f"[dim]Capture not stored: {e}[/dim]")
    else:
        if delivery.requested:
            render = {
                Format.XML: termstore.render,
                Format.MARKDOWN: termstore.render_markdown,
                Format.JSON: termstore.render_json,
            }[delivery.format]
            handler = OutputHandler(
                stdout=delivery.stdout, copy=delivery.copy, output=delivery.output
            )
            handler.handle_output(
                render([entry], include_context=False), item_name="1 terminal entry"
            )

    raise typer.Exit(code=exit_code)
