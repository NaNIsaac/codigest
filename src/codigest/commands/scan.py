import typer
import tomllib
from pathlib import Path
from rich.console import Console
from rich.progress import Progress, SpinnerColumn, TextColumn
from rich.panel import Panel
from rich.filesize import decimal

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
                console.print(f"[yellow]Warning: {p.name} is outside project root {root_path.name}[/yellow]")

    # Init check
    artifact_dir = root_path / ".codigest"
    if not artifact_dir.exists():
        console.print(f"[yellow].codigest directory missing in {root_path.name}. Running init...[/yellow]")
        try:
            artifact_dir.mkdir(exist_ok=True)
        except PermissionError:
            console.print(f"[red]Error: Cannot create .codigest at {root_path}[/red]")
            raise typer.Exit(1)

    output_path = artifact_dir / output
    prompt_engine = prompts.get_engine(root_path)
    anchor = shadow.ContextAnchor(root_path)

    extensions, extra_ignores = (None, [])
    if not all:
        extensions, extra_ignores = _load_config_filters(root_path)

    # ----------------------------------------------------------------
    # 1. Pre-flight Scan
    # ----------------------------------------------------------------
    with Progress(
        SpinnerColumn(),
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
        progress.update(task, completed=100)

    # Calculate Stats
    total_files = len(files)
    total_size = sum(f.stat().st_size for f in files)
    est_tokens = int(total_size / 4) 

    # Display Plan
    console.print(Panel(f"""[bold]Scan Plan[/bold]
  • Target: [cyan]{root_path}[/cyan]
  • Scope: {total_files} files
  • Est. Size: {decimal(total_size)}
  • Est. Tokens: ~{est_tokens:,}""", expand=False))

    # --- Smart Confirmation Logic ---
    TOKEN_THRESHOLD = 30000   # 30k tokens (Safe buffer for standard LLMs)
    FILE_COUNT_THRESHOLD = 100

    is_large_context = est_tokens > TOKEN_THRESHOLD or total_files > FILE_COUNT_THRESHOLD

    if yes:
        pass  # Explicit override
    elif is_large_context:
        console.print(f"[yellow]Large context detected (> {TOKEN_THRESHOLD:,} tokens or > {FILE_COUNT_THRESHOLD} files).[/yellow]")
        if not typer.confirm("Proceed with digestion?"):
            console.print("[red]Aborted.[/red]")
            raise typer.Exit()
    else:
        console.print("[dim]Small context detected. Automatically proceeding...[/dim]")

    # ----------------------------------------------------------------
    # 2. Execution (Heavy)
    # ----------------------------------------------------------------
    with Progress(
        SpinnerColumn(),
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
                
                # Using the unified tags.file factory
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
            console.print(f"[red]Template Rendering Failed:[/red] {e}")
            raise typer.Exit(1)

    # Final Actions
    try:
        anchor.update(files)
    except Exception as e:
        console.print(f"[yellow]Warning: Failed to update context anchor: {e}[/yellow]")

    try:
        output_path.write_text(snapshot_content, encoding="utf-8")

        # Final Verification
        final_token_count = tokenizer.estimate_tokens(snapshot_content)

        console.print("[bold green]✔ Snapshot Saved![/bold green]")
        console.print(f"  Path: [underline]{output_path}[/underline]")
        console.print(f"  Final Tokens: [bold cyan]~{final_token_count:,}[/bold cyan]")
        
        if anchor.has_history():
            pre_diff_path = artifact_dir / "previous_changes.diff"
            if pre_diff_path.exists() and pre_diff_path.stat().st_size > 0:
                console.print(f"  [dim]Changes before this scan saved to: {pre_diff_path.name}[/dim]")

    except Exception as e:
        console.print(f"[bold red]Save Failed:[/bold red] {e}")
        raise typer.Exit(1)