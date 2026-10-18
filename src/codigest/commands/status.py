from pathlib import Path

import typer
from rich.console import Console
from rich.markup import escape

lazy from ..core import common, shadow, termstore
lazy from . import pack

console = Console()


def handle(
    target: Path = typer.Argument(
        Path("."), help="Project directory", exists=True, file_okay=False, resolve_path=True
    ),
) -> None:
    """
    Show what `pack` would use here: the last failure, your changes and the baseline.
    """
    ctx = common.get_context(target)
    root = ctx.root_path
    files = ctx.get_target_files()
    entries = termstore.select(termstore.load_entries(), under=root)

    console.print(f"[bold]{escape(root.name)}[/bold]  [dim]{escape(str(root))}[/dim]", highlight=False)

    if failure := pack.pick_failure(entries, root):
        when = pack.ago(failure.started_at)
        console.print(f"  failure   [cyan]{escape(failure.command)}[/cyan] [dim](exit {failure.exit_code}, {when})[/dim]")
    else:
        hint = "run your tests through cdg cap" if not entries else f"{len(entries)} captured commands here"
        console.print(f"  failure   [dim]none in the last hour ({hint})[/dim]")

    diff, source, changed = pack.working_changes(root, files)
    if changed:
        console.print(f"  changes   {len(changed)} file{'s' * (len(changed) != 1)}, {source}")
        for path in changed[:10]:
            console.print(f"            [dim]{escape(path.relative_to(root).as_posix())}[/dim]", highlight=False)
        if len(changed) > 10:
            console.print(f"            [dim]... and {len(changed) - 10} more[/dim]")
    elif source:
        console.print("  changes   [dim]none[/dim]")
    else:
        console.print("  changes   [dim]not a Git repository; cdg mark sets a baseline to compare against[/dim]")

    anchor = shadow.ContextAnchor(root)
    if anchor.has_history():
        console.print(f"  baseline  set {anchor.get_last_update_time()} [dim](cdg mark moves it)[/dim]")
    else:
        console.print(f"  baseline  [dim]none{', so pack compares against Git' if source else ''}[/dim]")

    codigest_dir = root / ".codigest"
    if (codigest_dir / "config.toml").is_file():
        exts = ", ".join(sorted(ctx.config_extensions)) if ctx.config_extensions else "all"
        console.print(f"  filters   [dim].codigest/config.toml: extensions {exts}[/dim]")
    console.print(f"  files     [dim]{len(files)} considered[/dim]")
