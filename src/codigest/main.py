"""
Entry Point with Global Error Handling & Version Check
"""
import sys

import typer
from rich.console import Console

lazy from importlib.metadata import PackageNotFoundError, version

from .commands import init, scan, tree, digest, diff, semdiff, status, clean, cap, term

console = Console()

app = typer.Typer(
    name="codigest",
    help="Semantic Context Manager for LLM-assisted Development",
    add_completion=False,
    no_args_is_help=True
)

app.add_typer(init.app, name="init")
app.add_typer(status.app, name="status")
app.add_typer(clean.app, name="clean")
app.add_typer(scan.app, name="scan")
app.add_typer(tree.app, name="tree")
app.add_typer(digest.app, name="digest")
app.add_typer(diff.app, name="diff")
app.add_typer(semdiff.app, name="semdiff")
app.add_typer(term.app, name="term")

# `cap` is a plain command, not a group: its first argument is the wrapped
# program, which a Typer group would try to resolve as a subcommand.
app.command(name="cap", context_settings=cap.CONTEXT_SETTINGS)(cap.handle)

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
