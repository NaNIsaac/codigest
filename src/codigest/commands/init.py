from pathlib import Path

import typer
from rich.console import Console

lazy from ..core.common import hide_from_git

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
        resolve_path=True,
    ),
    force: bool = typer.Option(False, "--force", "-f", help="Overwrite existing config"),
) -> None:
    """
    Create .codigest/config.toml (file filters). `cdg mark` sets a baseline.
    """
    root_path = target  # Typer가 이미 resolve_path=True로 절대경로 변환해줌

    config_dir = root_path / ".codigest"
    config_file = config_dir / "config.toml"
    gitignore_file = root_path / ".gitignore"

    console.print(f"Setting up Codigest in {root_path}", highlight=False)

    hide_from_git(config_dir)
    if not config_file.exists() or force:
        config_file.write_text(DEFAULT_CONFIG, encoding="utf-8")
        console.print("  [green]Created[/green] .codigest/config.toml")
    else:
        console.print("  [yellow]Kept[/yellow] .codigest/config.toml (--force overwrites it)")

    _update_gitignore(gitignore_file)

    console.print("\nEdit .codigest/config.toml to choose which files are included.")
    console.print("[dim]To compare against this point later, run cdg mark.[/dim]")


def _update_gitignore(path: Path) -> None:
    entry = ".codigest/"
    if not path.exists():
        path.write_text(f"# Git Ignore\n{entry}\n", encoding="utf-8")
        return

    if entry not in path.read_text(encoding="utf-8"):
        with path.open("a", encoding="utf-8") as f:
            f.write(f"\n{entry}\n")
        console.print("  [green]Added[/green] .codigest/ to .gitignore")
