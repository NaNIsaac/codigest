"""
Entry Point with Global Error Handling & Version Check
"""
import sys
import typer
from rich.console import Console
from importlib.metadata import version, PackageNotFoundError

from .commands import init, scan, tree, digest, diff, semdiff

console = Console()

app = typer.Typer(
    name="codigest",
    help="Semantic Context Manager for LLM-assisted Development",
    add_completion=False,
    no_args_is_help=True
)

app.add_typer(init.app, name="init")
app.add_typer(scan.app, name="scan")
app.add_typer(tree.app, name="tree")
app.add_typer(digest.app, name="digest")
app.add_typer(diff.app, name="diff")
app.add_typer(semdiff.app, name="semdiff")

def version_callback(value: bool):
    """
    Callback function to handle --version flag.
    """
    if value:
        try:
            pkg_version = version("codigest")
        except PackageNotFoundError:
            pkg_version = "0.3.0 (dev)"
        
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
):
    pass

def main():
    try:
        app()
    except Exception as e:
        if isinstance(e, typer.Exit):
            raise e
        
        error_msg = str(e)
        console.print(f"[bold red]Error:[/bold red] {error_msg}")
        sys.exit(1)

if __name__ == "__main__":
    main()