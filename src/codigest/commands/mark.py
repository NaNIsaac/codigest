from pathlib import Path

import typer
from rich.console import Console

lazy from ..core import common, shadow

console = Console()


def handle(
    target: Path = typer.Argument(
        Path.cwd(), help="Project directory", exists=True, file_okay=False, resolve_path=True
    ),
) -> None:
    """
    Record the current state as the baseline for `diff`, `semdiff` and `pack`.
    """
    ctx = common.get_context(target)
    anchor = shadow.ContextAnchor(ctx.root_path)
    anchor.update(ctx.get_target_files())
    console.print("[green]Baseline set.[/green] [dim]`cdg diff` now shows changes from here.[/dim]")
