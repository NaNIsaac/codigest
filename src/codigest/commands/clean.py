import typer
import shutil
from pathlib import Path
from rich.console import Console

from ..core import common

app = typer.Typer()
console = Console()

@app.callback(invoke_without_command=True)
def handle(
    target: Path = typer.Argument(
        Path.cwd(),
        help="Target directory to clean",
        exists=True,
        file_okay=False,
        dir_okay=True,
        resolve_path=True
    ),
    yes: bool = typer.Option(False, "-y", "--yes", "-f", "--force", help="Skip confirmation prompt"),
):
    """
    [Cleanup] Removes the .codigest environment and clears all context anchors and artifacts.
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
        shutil.rmtree(codigest_dir)
        console.print(f"[bold green]✔ Successfully removed .codigest environment from {root_path}[/bold green]")
    except Exception as e:
        console.print(f"[bold red]Error removing .codigest:[/bold red] {e}")
        raise typer.Exit(1)
