from pathlib import Path

import typer
from rich.console import Console

lazy from ..core import common

app = typer.Typer()
console = Console()

@app.callback(invoke_without_command=True)
def handle(
    target: Path = typer.Argument(
        Path("."),
        help="Target directory to clean",
        exists=True,
        file_okay=False,
        dir_okay=True,
        resolve_path=True
    ),
    yes: bool = typer.Option(False, "-y", "--yes", "-f", "--force", help="Skip confirmation prompt"),
) -> None:
    """
    Delete .codigest/: the config, the baseline and the cache.
    """
    ctx = common.get_context(target)
    root_path = ctx.root_path
    codigest_dir = root_path / ".codigest"

    if not codigest_dir.exists():
        console.print(f"[yellow]No .codigest directory found in {root_path}[/yellow]")
        return

    if not yes:
        console.print(f"[yellow]Warning: This will remove [bold]{codigest_dir}[/bold] and all stored context/anchors.[/yellow]")
        if not typer.confirm("Are you sure you want to remove .codigest?"):
            console.print("[red]Aborted.[/red]")
            raise typer.Exit()

    try:
        common.remove_tree(codigest_dir)
        console.print(f"[green]Removed[/green] {root_path / '.codigest'}")
    except Exception as e:
        console.print(f"[bold red]Error removing .codigest:[/bold red] {e}")
        raise typer.Exit(1) from e
