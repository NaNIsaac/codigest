"""
Entry Point with Global Error Handling & Version Check
"""
import sys

import typer
from loguru import logger
from rich.console import Console

lazy from importlib.metadata import PackageNotFoundError, version

from .commands import init, scan, tree, digest, diff, semdiff, status, clean, cap, term, mcp, pack, mark

console = Console()

# loguru logs everything from DEBUG up by default; that is noise in a CLI.
logger.remove()
logger.add(sys.stderr, level="WARNING")

app = typer.Typer(
    name="codigest",
    help="Share the code context you choose with an AI: the project, a few files, what changed, or a failing command.",
    add_completion=False,
    no_args_is_help=True
)

# `cdg --help` groups commands by what you are trying to do, in the order most people need them.
SHARE, CHANGES, TERMINAL, AGENTS, SETUP = (
    "Share code", "Share changes", "Share a failing command", "Agents", "Setup",
)
app.command(name="scan", rich_help_panel=SHARE)(scan.handle)
app.command(name="digest", rich_help_panel=SHARE)(digest.handle)
app.command(name="tree", rich_help_panel=SHARE)(tree.handle)
app.command(name="mark", rich_help_panel=CHANGES)(mark.handle)
app.command(name="diff", rich_help_panel=CHANGES)(diff.handle)
app.command(name="semdiff", rich_help_panel=CHANGES)(semdiff.handle)
# `cap` is a plain command, not a group: its first argument is the wrapped
# program, which a Typer group would try to resolve as a subcommand.
app.command(name="cap", context_settings=cap.CONTEXT_SETTINGS, rich_help_panel=TERMINAL)(cap.handle)
app.add_typer(term.app, name="term", rich_help_panel=TERMINAL)
app.command(name="pack", rich_help_panel=TERMINAL)(pack.handle)
app.command(name="mcp", rich_help_panel=AGENTS)(mcp.handle)
app.command(name="status", rich_help_panel=SETUP)(status.handle)
app.command(name="init", rich_help_panel=SETUP)(init.handle)
app.command(name="clean", rich_help_panel=SETUP)(clean.handle)

def version_callback(value: bool) -> None:
    """
    Callback function to handle --version flag.
    """
    if value:
        try:
            pkg_version = version("codigest")
        except PackageNotFoundError:
            pkg_version = "dev"

        console.print(f"codigest {pkg_version}")
        raise typer.Exit()

@app.callback()
def common(
    ctx: typer.Context,
    version: bool = typer.Option(
        None,
        "--version", "-v",
        help="Show the application version and exit.",
        callback=version_callback,
        is_eager=True
    )
) -> None:
    pass

def main() -> None:
    try:
        app()
    except typer.Exit:
        raise
    except Exception as e:
        console.print(f"[bold red]Error:[/bold red] {e}")
        sys.exit(1)

if __name__ == "__main__":
    main()
