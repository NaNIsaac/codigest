import typer
from pathlib import Path
from rich.console import Console
from rich.tree import Tree
from rich.text import Text
from rich.filesize import decimal

from ..core import common

app = typer.Typer()
console = Console()

def _build_rich_tree(root_path: Path, files: list[Path]) -> Tree:
    # (기존의 이모티콘 없는 버전 로직 유지)
    tree = Tree(f"[bold blue]{root_path.name}[/bold blue]", guide_style="bold bright_black")
    dir_nodes = {root_path: tree}

    for path in files:
        # (중략: 기존 트리 생성 로직)
        # External path 처리 (루트 밖의 파일)
        try:
            relative = path.relative_to(root_path)
            parts = relative.parts
            current_node = tree
            current_path = root_path

            for part in parts[:-1]:
                current_path = current_path / part
                if current_path not in dir_nodes:
                    dir_nodes[current_path] = current_node.add(f"[bold cyan]{part}[/bold cyan]")
                current_node = dir_nodes[current_path]
            
            filename = parts[-1]
        except ValueError:
            # 루트 밖의 파일은 별도 노드로 표시하지 않고 루트에 [EXTERNAL] 접두어로 추가
            filename = f"[EXTERNAL] {path.name}"
            current_node = tree 

        stat = path.stat()
        size_str = decimal(stat.st_size)
        
        style = "white"
        if filename.endswith(".py"): style = "green"
        elif filename.endswith((".js", ".ts")): style = "yellow"
        elif filename.endswith((".json", ".toml")): style = "blue"
        elif filename.endswith(".md"): style = "cyan"

        label = Text(f"{filename}", style=style)
        label.append(f" ({size_str})", style="dim")
        current_node.add(label)

    return tree

@app.callback(invoke_without_command=True)
def handle(
    target: Path = typer.Argument(Path.cwd(), help="Target directory"),
    all: bool = typer.Option(False, "--all", "-a", help="Ignore config filters"),
    resolve: bool = typer.Option(False, "-r", "--resolve", help="Recursively resolve imports"),
):
    """
    [Visual] Print the project directory tree.
    """
    # 1. Context 생성 (루트 찾기용)
    ctx = common.get_context(target)
    root_path = ctx.root_path

    # 2. 파일 확보 (Scope 지정!)
    try:
        # [수정] targets=[target]을 명시적으로 전달해야 해당 폴더만 스캔함
        files = ctx.get_target_files(targets=[target], ignore_config=all, resolve_deps=resolve)
    except Exception as e:
        console.print(f"[red][Error] Scan failed:[/red] {e}")
        raise typer.Exit(code=1)

    if not files:
        console.print("[yellow][Warning] No matching files found.[/yellow]")
        return

    # 3. Visualize
    tree_viz = _build_rich_tree(root_path, files)
    console.print(tree_viz)
    console.print(f"\n[dim]Found {len(files)} files.[/dim]")