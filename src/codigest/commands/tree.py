import typer
from pathlib import Path
from rich.tree import Tree
from rich.text import Text
from rich.filesize import decimal

from ..options import CopyOption, StdoutOption, ResolveOption, AllOption
from ..core import common, structure
from ..core.output import OutputHandler

app = typer.Typer()

def _build_rich_tree(root_path: Path, files: list[Path]) -> Tree:
    # (기존의 이모티콘 없는 버전 로직 유지)
    tree = Tree(f"[bold blue]{root_path.name}[/bold blue]", guide_style="bold bright_black")
    dir_nodes = {root_path: tree}

    for path in files:
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
    copy: CopyOption = False,
    stdout: StdoutOption = False,
    all: AllOption = False,
    resolve: ResolveOption = False,
):
    """
    [Visual] Print the project directory tree.
    """
    handler = OutputHandler(stdout=stdout, copy=copy)
    log_console = handler.log_console

    # 1. Context 생성 (루트 찾기용)
    ctx = common.get_context(target)
    root_path = ctx.root_path

    # 2. 파일 확보 (Scope 지정!)
    try:
        # [수정] targets=[target]을 명시적으로 전달해야 해당 폴더만 스캔함
        files = ctx.get_target_files(targets=[target], ignore_config=all, resolve_deps=resolve)
    except Exception as e:
        log_console.print(f"[red][Error] Scan failed:[/red] {e}")
        raise typer.Exit(code=1)

    if not files:
        log_console.print("[yellow][Warning] No matching files found.[/yellow]")
        return

    # 3. Visualize
    ascii_tree = structure.generate_ascii_tree(files, root_path)
    
    def _print_tree_viz():
        tree_viz = _build_rich_tree(root_path, files)
        log_console.print(tree_viz)
        log_console.print(f"\n[dim]Found {len(files)} files.[/dim]")

    handler.handle_output(ascii_tree, save_action=_print_tree_viz, item_name="tree")