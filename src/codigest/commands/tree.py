from pathlib import Path

import typer

from ..options import CopyOption, StdoutOption, ResolveOption, AllOption, OutputOption

lazy from rich.tree import Tree
lazy from rich.text import Text
lazy from rich.filesize import decimal
lazy from ..core import common, structure
lazy from ..core.output import OutputHandler

app = typer.Typer()


def _build_rich_tree(root_path: Path, files: list[Path]) -> Tree:
    tree = Tree(f"[bold blue]{root_path.name}[/bold blue]", guide_style="bold bright_black")
    dir_nodes = {root_path: tree}

    for path in files:
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
            filename = f"[EXTERNAL] {path.name}"
            current_node = tree

        stat = path.stat()
        size_str = decimal(stat.st_size)

        match path.suffix:
            case ".py":
                style = "green"
            case ".js" | ".ts":
                style = "yellow"
            case ".json" | ".toml":
                style = "blue"
            case ".md":
                style = "cyan"
            case _:
                style = "white"

        label = Text(filename, style=style)
        label.append(f" ({size_str})", style="dim")
        current_node.add(label)

    return tree


@app.callback(invoke_without_command=True)
def handle(
    target: Path = typer.Argument(Path("."), help="Target directory"),
    copy: CopyOption = False,
    output: OutputOption = None,
    stdout: StdoutOption = False,
    all: AllOption = False,
    resolve: ResolveOption = False,
    depth: int | None = typer.Option(
        None,
        "-L",
        "--depth",
        min=1,
        help="Show this many levels; deeper folders become a file count",
    ),
) -> None:
    """
    Print the project file tree.
    """
    handler = OutputHandler(stdout=stdout, copy=copy, output=output)
    log_console = handler.log_console

    ctx = common.get_context(target)
    root_path = ctx.root_path

    try:
        # Without explicit targets the whole root is scanned.
        files = ctx.get_target_files(targets=[target], ignore_config=all, resolve_deps=resolve)
    except Exception as e:
        log_console.print(f"[red][Error] Scan failed:[/red] {e}")
        raise typer.Exit(code=1) from e

    if not files:
        log_console.print("[yellow][Warning] No matching files found.[/yellow]")
        return

    ascii_tree = structure.generate_ascii_tree(files, root_path, depth)

    def _print_tree_viz():
        if depth is not None:
            log_console.print(ascii_tree, highlight=False)
            return
        tree_viz = _build_rich_tree(root_path, files)
        log_console.print(tree_viz)
        log_console.print(f"\n[dim]Found {len(files)} files.[/dim]")

    handler.handle_output(ascii_tree, summary=_print_tree_viz, item_name="tree")
