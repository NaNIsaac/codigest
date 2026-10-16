from pathlib import Path

import typer
from rich.console import Console

lazy from rich.panel import Panel
lazy from ..core import scanner, shadow

app = typer.Typer()
console = Console()

DEFAULT_CONFIG = """# Codigest Configuration File
[project]
description = "Auto-generated context configuration"

[filter]
max_file_size_kb = 100
extensions = [
    ".py", ".pyi",
    ".ts", ".tsx", ".js", ".jsx",
    ".json", ".html", ".css",
    ".md", ".toml", ".yaml", ".yml", ".xml"
]
exclude_patterns = [
    "*.lock",
    "dist/",
    "build/",
    "node_modules/",
    "__pycache__/"
]

[output]
format = "xml"
structure = "toon"
"""

@app.callback(invoke_without_command=True)
def handle(
    # target 인자 추가 (기본값: 현재 위치)
    target: Path = typer.Argument(
        Path("."),
        help="Target directory to initialize",
        exists=True,
        file_okay=False,
        dir_okay=True,
        resolve_path=True
    ),
    force: bool = typer.Option(False, "--force", "-f", help="Overwrite existing config"),
) -> None:
    """
    Create .codigest/config.toml (file filters) and set a baseline.
    """
    root_path = target  # Typer가 이미 resolve_path=True로 절대경로 변환해줌

    config_dir = root_path / ".codigest"
    config_file = config_dir / "config.toml"
    gitignore_file = root_path / ".gitignore"

    console.print(Panel(f"[bold blue]Initializing Codigest[/bold blue]\n📂 Location: {root_path}", expand=False))

    # 1. Config Setup (기존 로직)
    if not config_dir.exists():
        config_dir.mkdir()
        console.print("  [green]✔[/green] Created [bold].codigest/[/bold] directory")

    if not config_file.exists() or force:
        config_file.write_text(DEFAULT_CONFIG, encoding="utf-8")
        console.print("  [green]✔[/green] Created [bold]config.toml[/bold]")
    else:
        console.print("  [yellow]![/yellow] Config exists. Use --force to overwrite.")

    _update_gitignore(gitignore_file)

    # 2. [NEW] Initial Anchoring (Shadow Git)
    #    "프로그램 시작 시에도 Anchor 하나 잡고 시작"
    console.print("\n[dim]Creating initial context anchor...[/dim]")
    try:
        # Default scan settings for initialization
        files = scanner.scan_project(root_path)
        anchor = shadow.ContextAnchor(root_path)
        anchor.update(files)
        console.print("  [green]✔[/green] Baseline snapshot captured.")
    except Exception as e:
        console.print(f"  [red]⚠️ Failed to create initial anchor: {e}[/red]")

    console.print("\n[bold green]✨ Ready to digest![/bold green]")
    console.print("Try running: [cyan]codigest diff[/cyan] to see changes from now on.")

def _update_gitignore(path: Path) -> None:
    entry = ".codigest/"
    if not path.exists():
        path.write_text(f"# Git Ignore\n{entry}\n", encoding="utf-8")
        return

    if entry not in path.read_text(encoding="utf-8"):
        with path.open("a", encoding="utf-8") as f:
            f.write(f"\n{entry}\n")
        console.print("  [green]✔[/green] Added [bold].codigest/[/bold] to .gitignore")