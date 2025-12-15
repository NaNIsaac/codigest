import typer
import tomllib
from pathlib import Path
from rich.console import Console
from rich.progress import Progress, SpinnerColumn, TextColumn

from ..core import scanner, structure, tags, prompts, processor, shadow, tokenizer

app = typer.Typer()
console = Console()

def _load_config_filters(root_path: Path):
    config_path = root_path / ".codigest" / "config.toml"
    extensions = None
    exclude_patterns = []
    
    if config_path.exists():
        try:
            with open(config_path, "rb") as f:
                data = tomllib.load(f)
                filters = data.get("filter", {})
                ext_list = filters.get("extensions", [])
                if ext_list:
                    extensions = set(ext_list)
                exclude_patterns = filters.get("exclude_patterns", [])
        except Exception:
            pass 
            
    return extensions, exclude_patterns

def _find_project_root(start_path: Path) -> Path:
    """
    .codigest 폴더나 .git 폴더가 있는 상위 디렉토리를 찾습니다.
    못 찾으면 start_path를 반환합니다.
    """
    current = start_path.resolve()
    for parent in [current] + list(current.parents):
        if (parent / ".codigest").exists() or (parent / ".git").exists():
            return parent
    return start_path

@app.callback(invoke_without_command=True)
def handle(
    # [변경] targets를 리스트로 받음 (여러 폴더 지정 가능)
    targets: list[Path] = typer.Argument(
        None, 
        help="Specific files or directories to scan (Scope)",
        exists=True,
        resolve_path=True
    ),
    output: str = typer.Option("snapshot.xml", help="Output filename inside .codigest/"),
    all: bool = typer.Option(False, "--all", "-a", help="Ignore config filters"),
    message: str = typer.Option("", "--message", "-m", help="Add specific instruction"),
    line_numbers: bool = typer.Option(False, "--lines", "-l", help="Add line numbers to code blocks"),
):
    """
    Scans the codebase. 
    If TARGETS provided, only scans those paths within the project.
    """
    # 1. 프로젝트 루트 결정 (현재 위치 기준 상위 탐색)
    root_path = _find_project_root(Path.cwd())
    
    # 2. Scope 설정
    # 입력된 targets가 있으면 사용, 없으면 [root_path] 전체
    scan_scope = targets if targets else None

    # 검증: targets가 root_path 밖에 있으면 경고 (옵션)
    if scan_scope:
        for p in scan_scope:
            if not p.is_relative_to(root_path):
                console.print(f"[yellow]⚠️  Warning: {p.name} is outside project root {root_path.name}[/yellow]")

    artifact_dir = root_path / ".codigest"

    if not artifact_dir.exists():
        console.print(f"[yellow]⚠️  .codigest directory missing in {root_path.name}. Running init...[/yellow]")
        try:
            artifact_dir.mkdir(exist_ok=True)
        except PermissionError:
            console.print(f"[red]❌ Error: Cannot create .codigest at {root_path}[/red]")
            raise typer.Exit(1)

    output_path = artifact_dir / output

    # Initialize Engines
    prompt_engine = prompts.get_engine(root_path)
    anchor = shadow.ContextAnchor(root_path)

    extensions, extra_ignores = (None, [])
    if not all:
        extensions, extra_ignores = _load_config_filters(root_path)

    with Progress(
        SpinnerColumn(),
        TextColumn("[bold blue]Scanning Project...[/bold blue]"),
        transient=True,
        console=console
    ):

        # File Discovery
        files = scanner.scan_project(
            root_path, 
            extensions, 
            extra_ignores, 
            include_paths=scan_scope
        )

        # Pre-scan Diff Report
        if anchor.has_history():
            try:
                diff_content = anchor.get_changes(files)
                if diff_content.strip():
                    pre_diff_path = artifact_dir / "previous_changes.diff"
                    pre_diff_path.write_text(diff_content, encoding="utf-8")
            except Exception:
                pass

        # Structure Generation
        tree_str = structure.generate_ascii_tree(files, root_path)

        # Content Processing
        file_blocks = []
        for file_path in files:
            rel_path = file_path.relative_to(root_path).as_posix()
            try:
                content = processor.read_file_content(file_path, add_line_numbers=line_numbers)
                block = tags.xml(t"""
                    <file path="{rel_path}">
                    {content}
                    </file>
                """)
                file_blocks.append(block)
            except Exception:
                continue

        source_code_blob = "\n\n".join(file_blocks)

        # Final Assembly
        try:
            snapshot_content = prompt_engine.render(
                "snapshot",
                project_name=root_path.name,
                tree_structure=tree_str,
                source_code=source_code_blob,
                instruction=message
            )
        except Exception as e:
            console.print(f"[red]❌ Template Rendering Failed:[/red] {e}")
            raise typer.Exit(1)

    # Update Anchor
    try:
        anchor.update(files)
    except Exception as e:
        console.print(f"[yellow]⚠️  Warning: Failed to update context anchor: {e}[/yellow]")

    # Save to Disk
    try:
        output_path.write_text(snapshot_content, encoding="utf-8")

        size_kb = len(snapshot_content) / 1024
        token_count = tokenizer.estimate_tokens(snapshot_content)
        
        # Report
        console.print("[bold green]✔ Snapshot Saved![/bold green]")
        console.print(f"  📄 Path: [underline]{output_path}[/underline]")
        console.print(f"  📊 Size: {size_kb:.1f} KB | [bold cyan]~{token_count:,} Tokens[/bold cyan]")
        console.print(f"  📂 Files: {len(files)}")
        
        if anchor.has_history():
            pre_diff_path = artifact_dir / "previous_changes.diff"
            if pre_diff_path.exists() and pre_diff_path.stat().st_size > 0:
                console.print(f"  [dim]ℹ️  Changes before this scan saved to: {pre_diff_path.name}[/dim]")

    except Exception as e:
        console.print(f"[bold red]❌ Save Failed:[/bold red] {e}")
        raise typer.Exit(1)