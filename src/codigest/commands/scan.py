import typer
import tomllib
from pathlib import Path
from rich.console import Console
from rich.progress import Progress, SpinnerColumn, TextColumn
from rich.panel import Panel
from rich.filesize import decimal

# [변경] resolver 추가
from ..core import scanner, structure, tags, prompts, processor, shadow, tokenizer, resolver

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
    current = start_path.resolve()
    for parent in [current] + list(current.parents):
        if (parent / ".codigest").exists() or (parent / ".git").exists():
            return parent
    return start_path

@app.callback(invoke_without_command=True)
def handle(
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
    yes: bool = typer.Option(False, "-y", "--yes", help="Skip confirmation prompt"),
    # [추가] 의존성 해결 옵션
    resolve: bool = typer.Option(False, "-r", "--resolve", help="Recursively resolve imports for local files"),
):
    """
    Scans the codebase. 
    If TARGETS provided, only scans those paths within the project.
    """
    root_path = _find_project_root(Path.cwd())
    scan_scope = targets if targets else None

    # Scope validation
    if scan_scope:
        for p in scan_scope:
            if not p.is_relative_to(root_path):
                # [수정] 이모티콘 제거
                console.print(f"[yellow][Warning] {p.name} is outside project root {root_path.name}[/yellow]")

    # Init check
    artifact_dir = root_path / ".codigest"
    if not artifact_dir.exists():
        # [수정] 이모티콘 제거
        console.print(f"[yellow][Warning] .codigest directory missing in {root_path.name}. Running init...[/yellow]")
        try:
            artifact_dir.mkdir(exist_ok=True)
        except PermissionError:
            # [수정] 이모티콘 제거
            console.print(f"[red][Error] Cannot create .codigest at {root_path}[/red]")
            raise typer.Exit(1)

    output_path = artifact_dir / output
    prompt_engine = prompts.get_engine(root_path)
    anchor = shadow.ContextAnchor(root_path)

    extensions, extra_ignores = (None, [])
    if not all:
        extensions, extra_ignores = _load_config_filters(root_path)

    # ----------------------------------------------------------------
    # 1. Pre-flight Scan (Fast)
    # ----------------------------------------------------------------
    with Progress(
        SpinnerColumn(),
        # [수정] 이모티콘 제거
        TextColumn("[bold blue]Scanning file structure...[/bold blue]"),
        transient=True,
        console=console
    ) as progress:
        task = progress.add_task("scanning", total=None)
        files = scanner.scan_project(
            root_path, 
            extensions, 
            extra_ignores, 
            include_paths=scan_scope
        )
        
        # [추가] 의존성 해결 로직 (Resolver)
        if resolve:
            progress.update(task, description="[bold blue]Resolving dependencies...[/bold blue]")
            initial_count = len(files)
            files = resolver.resolve_dependencies(root_path, files)
            resolved_count = len(files)
            if resolved_count > initial_count:
                console.print(f"[dim]Resolved {resolved_count - initial_count} dependency files.[/dim]")

        progress.update(task, completed=100)

    # Calculate Stats
    total_files = len(files)
    total_size = sum(f.stat().st_size for f in files)
    est_tokens = int(total_size / 4) 

    # Display Plan
    # [수정] 이모티콘 제거 및 레이아웃 정리
    console.print(Panel(f"""[bold]Scan Plan[/bold]
  Target: [cyan]{root_path}[/cyan]
  Scope: {total_files} files
  Est. Size: {decimal(total_size)}
  Est. Tokens: ~{est_tokens:,}""", expand=False))

    # --- Smart Confirmation Logic ---
    TOKEN_THRESHOLD = 30000   
    FILE_COUNT_THRESHOLD = 100

    is_large_context = est_tokens > TOKEN_THRESHOLD or total_files > FILE_COUNT_THRESHOLD

    if yes:
        pass  # Explicit override
    elif is_large_context:
        # [수정] 이모티콘 제거
        console.print(f"[yellow][Warning] Large context detected (> {TOKEN_THRESHOLD:,} tokens or > {FILE_COUNT_THRESHOLD} files).[/yellow]")
        # [수정] 이모티콘 제거
        if not typer.confirm("Proceed with digestion?"):
            console.print("[red]Aborted.[/red]")
            raise typer.Exit()
    else:
        # [수정] 이모티콘 제거
        console.print("[dim]Small context detected. Automatically proceeding...[/dim]")

    # ----------------------------------------------------------------
    # 2. Execution (Heavy)
    # ----------------------------------------------------------------
    with Progress(
        SpinnerColumn(),
        # [수정] 이모티콘 제거
        TextColumn("[bold blue]Generating Snapshot...[/bold blue]"),
        transient=True,
        console=console
    ) as progress:
        
        # Diff Report
        if anchor.has_history():
            try:
                diff_content = anchor.get_changes(files)
                if diff_content.strip():
                    pre_diff_path = artifact_dir / "previous_changes.diff"
                    pre_diff_path.write_text(diff_content, encoding="utf-8")
            except Exception:
                pass

        # Tree Generation
        tree_str = structure.generate_ascii_tree(files, root_path)

        # Content Reading & Tagging
        file_blocks = []
        for file_path in files:
            rel_path = file_path.relative_to(root_path).as_posix()
            try:
                content = processor.read_file_content(file_path, add_line_numbers=line_numbers)
                
                block = tags.file(rel_path, content)
                file_blocks.append(block)
            except Exception:
                continue

        source_code_blob = "\n\n".join(file_blocks)

        # Template Rendering
        try:
            snapshot_content = prompt_engine.render(
                "snapshot",
                project_name=root_path.name,
                tree_structure=tree_str,
                source_code=source_code_blob,
                instruction=message
            )
        except Exception as e:
            # [수정] 이모티콘 제거
            console.print(f"[red][Error] Template Rendering Failed:[/red] {e}")
            raise typer.Exit(1)

    # Final Actions
    try:
        anchor.update(files)
    except Exception as e:
        # [수정] 이모티콘 제거
        console.print(f"[yellow][Warning] Failed to update context anchor: {e}[/yellow]")

    try:
        output_path.write_text(snapshot_content, encoding="utf-8")

        final_token_count = tokenizer.estimate_tokens(snapshot_content)

        # [수정] 이모티콘 제거 및 메시지 단순화
        console.print("[bold green]Snapshot Saved![/bold green]")
        console.print(f"  Path: [underline]{output_path}[/underline]")
        console.print(f"  Final Tokens: [bold cyan]~{final_token_count:,}[/bold cyan]")
        
        if anchor.has_history():
            pre_diff_path = artifact_dir / "previous_changes.diff"
            if pre_diff_path.exists() and pre_diff_path.stat().st_size > 0:
                # [수정] 이모티콘 제거
                console.print(f"  [dim]Changes before this scan saved to: {pre_diff_path.name}[/dim]")

    except Exception as e:
        # [수정] 이모티콘 제거
        console.print(f"[bold red][Error] Save Failed:[/bold red] {e}")
        raise typer.Exit(1)