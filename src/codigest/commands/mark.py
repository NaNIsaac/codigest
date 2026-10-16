from pathlib import Path

import typer
from rich.console import Console

lazy from ..core import common, shadow

console = Console()


def handle(
    target: Path = typer.Argument(
        Path("."), help="Project directory", exists=True, file_okay=False, resolve_path=True
    ),
) -> None:
    """
    Set the baseline that `diff`, `semdiff` and `pack` compare against.
    """
    ctx = common.get_context(target)
    anchor = shadow.ContextAnchor(ctx.root_path)
    anchor.update(ctx.get_target_files())
    console.print("[green]Baseline set.[/green] [dim]`cdg diff` now shows changes from here.[/dim]")
