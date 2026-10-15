"""
Entry Point with Global Error Handling & Version Check
"""
import sys

import typer
from rich.console import Console

lazy from importlib.metadata import PackageNotFoundError, version

from .commands import init, scan, tree, digest, diff, semdiff, status, clean, cap, term, mcp

console = Console()

app = typer.Typer(
    name="codigest",
    help="Semantic Context Manager for LLM-assisted Development",
    add_completion=False,
    no_args_is_help=True
)

app.command(name="init")(init.handle)
app.command(name="status")(status.handle)
app.command(name="clean")(clean.handle)
app.command(name="scan")(scan.handle)
app.command(name="tree")(tree.handle)
app.command(name="digest")(digest.handle)
app.command(name="diff")(diff.handle)
app.command(name="semdiff")(semdiff.handle)
app.add_typer(term.app, name="term")

# `cap` is a plain command, not a group: its first argument is the wrapped
# program, which a Typer group would try to resolve as a subcommand.
app.command(name="cap", context_settings=cap.CONTEXT_SETTINGS)(cap.handle)
app.command(name="mcp")(mcp.handle)

def version_callback(value: bool) -> None:
    """
    Callback function to handle --version flag.
    """
    if value:
        try:
            pkg_version = version("codigest")
        except PackageNotFoundError:
            pkg_version = "dev"

        console.print(f"🦁 [bold cyan]Codigest[/bold cyan] version [bold green]{pkg_version}[/bold green]")
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
