import typer
import tomllib
from pathlib import Path
from typing import Optional
from rich.console import Console
from rich.tree import Tree
from rich.text import Text
from rich.filesize import decimal

from ..core.scanner import scan_project

app = typer.Typer()
console = Console()

def _load_extensions_from_config(root_path: Path) -> Optional[set[str]]:
    """Attempts to load target extensions from .codigest/config.toml"""
    config_path = root_path / ".codigest" / "config.toml"
    if not config_path.exists():
        return None
    try:
        with open(config_path, "rb") as f:
            data = tomllib.load(f)
            exts = data.get("filter", {}).get("extensions", [])
            return set(exts) if exts else None
    except Exception:
        return None

def _build_rich_tree(root_path: Path, files: list[Path]) -> Tree:
    """Converts a flat list of Paths into a visual Rich Tree."""
    tree = Tree(
        f":open_file_folder: [bold blue]{root_path.name}[/bold blue]",
        guide_style="bold bright_black",
    )
    dir_nodes = {root_path: tree}

    for path in files:
        relative = path.relative_to(root_path)
        parts = relative.parts
        current_node = tree
        current_path = root_path
        
        for part in parts[:-1]:
            current_path = current_path / part
            if current_path not in dir_nodes:
                dir_nodes[current_path] = current_node.add(f":folder: [bold cyan]{part}[/bold cyan]")
            current_node = dir_nodes[current_path]
        
        stat = path.stat()
        size_str = decimal(stat.st_size)
        filename = parts[-1]
        
        # Icon logic
        icon, style = "📄", "white"
        if filename.endswith(".py"):
            icon, style = "🐍", "green"
        elif filename.endswith((".js", ".ts", ".tsx")):
            icon, style = "📜", "yellow"
        elif filename.endswith((".html", ".css")):
            icon, style = "🌐", "magenta"
        elif filename.endswith((".json", ".toml", ".yaml")):
            icon, style = "⚙️", "blue"
        elif filename.endswith(".md"):
            icon, style = "📝", "cyan"

        label = Text(f"{icon} {filename}", style=style)
        label.append(f" ({size_str})", style="dim")
        current_node.add(label)

    return tree

@app.callback(invoke_without_command=True)
def handle(
    target: Path = typer.Argument(Path.cwd(), help="Target directory"),
    all: bool = typer.Option(False, "--all", "-a", help="Ignore config filters"),
):
    """
    [Visual] Print the project directory tree (Respects .gitignore).
    """
    if not target.exists():
        console.print(f"[red]Error: Path '{target}' does not exist.[/red]")
        raise typer.Exit(code=1)

    root_path = target.resolve()
    
    # 1. Determine Filters
    extensions = None
    if not all:
        extensions = _load_extensions_from_config(root_path)
        filter_status = f"[dim]Filter: {len(extensions)} extensions from config[/dim]" if extensions else "[dim]Filter: Default safelist[/dim]"
    else:
        filter_status = "[bold yellow]Filter: ALL (Gitignore only)[/bold yellow]"

    console.print(f"[bold]🔍 Scanning:[/bold] {root_path}")
    console.print(filter_status)

    # 2. Scan
    try:
        files = scan_project(root_path, extensions)
    except Exception as e:
        console.print(f"[red]Scan failed:[/red] {e}")
        raise typer.Exit(code=1)

    if not files:
        console.print("[yellow]⚠️ No matching files found.[/yellow]")
        return

    # 3. Visualize
    tree_viz = _build_rich_tree(root_path, files)
    console.print(tree_viz)
    console.print(f"\n[dim]Found {len(files)} files.[/dim]")