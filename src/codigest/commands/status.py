import typer
from pathlib import Path
from rich.console import Console
from rich.panel import Panel
from rich.table import Table
from rich.filesize import decimal

from ..core import common, shadow, tokenizer

app = typer.Typer()
console = Console()

@app.callback(invoke_without_command=True)
def handle(
    target: Path = typer.Argument(
        Path.cwd(),
        help="Target directory to check status",
        exists=True,
        file_okay=False,
        dir_okay=True,
        resolve_path=True
    ),
    resolve: bool = typer.Option(False, "-r", "--resolve", help="Recursively resolve imports when checking context files"),
):
    """
    [Environment Status] Displays information about the .codigest setup, anchor baseline, changes, and artifacts.
    """
    ctx = common.get_context(target)
    root_path = ctx.root_path
    codigest_dir = root_path / ".codigest"
    
    is_initialized = codigest_dir.exists() and codigest_dir.is_dir()
    
    # Header Panel
    init_status_str = "[bold green]Initialized[/bold green]" if is_initialized else "[bold yellow]Not Initialized[/bold yellow]"
    header_content = (
        f"🦁 [bold cyan]Codigest Environment Status[/bold cyan]\n"
        f"📂 [bold]Root:[/bold] {root_path}\n"
        f"⚙️ [bold]Status:[/bold] {init_status_str} "
        f"({'[dim].codigest/ present[/dim]' if is_initialized else '[dim]run cdg init to set up[/dim]'})"
    )
    console.print(Panel(header_content, expand=False))

    if not is_initialized:
        console.print("\n[yellow]💡 Tip: Run [bold cyan]cdg init[/bold cyan] to initialize context tracking for this repository.[/yellow]")
        return

    anchor = shadow.ContextAnchor(root_path)
    has_anchor = anchor.has_history()
    last_update = anchor.get_last_update_time() if has_anchor else "Never"

    # Fetch target files
    current_files = ctx.get_target_files(resolve_deps=resolve)
    total_files = len(current_files)

    # 1. Baseline Section
    console.print("\n[bold blue]📍 Context Anchor Baseline[/bold blue]")
    console.print(f"  • [bold]Last Snapshot:[/bold] {last_update}")
    console.print(f"  • [bold]Tracked Source Files:[/bold] {total_files} files")

    # 2. Pending Working Tree Changes
    console.print("\n[bold blue]📝 Pending Working Tree Changes[/bold blue] [dim](since last scan)[/dim]")
    if not has_anchor:
        console.print("  [yellow]⚠️ No baseline scan found. Run [bold cyan]cdg scan[/bold cyan] to create the initial snapshot.[/yellow]")
    else:
        diff_content = anchor.get_changes(current_files)
        changed_files = anchor.get_changed_files(current_files)

        if not diff_content.strip():
            console.print("  [green]✔ Working tree clean (No pending changes detected)[/green]")
        else:
            diff_tokens = tokenizer.estimate_tokens(diff_content)
            console.print(f"  • [bold]Changed Files:[/bold] [yellow]{len(changed_files)} files[/yellow]")
            console.print(f"  • [bold]Est. Diff Size:[/bold] [cyan]~{diff_tokens:,} tokens[/cyan] ({len(diff_content):,} chars)")
            
            console.print("  • [bold]Modified / New Files:[/bold]")
            max_show = 10
            for idx, p in enumerate(changed_files):
                if idx >= max_show:
                    remaining = len(changed_files) - max_show
                    console.print(f"    [dim]... and {remaining} more file(s)[/dim]")
                    break
                try:
                    rel_p = p.relative_to(root_path).as_posix()
                except ValueError:
                    rel_p = str(p)
                
                # Check if new file or modified
                try:
                    rel_path_obj = p.relative_to(root_path)
                    anchor_file_content = anchor.read_anchor_file(rel_path_obj)
                except Exception:
                    anchor_file_content = ""

                status_label = "[green][NEW]     [/green]" if not anchor_file_content else "[yellow][MODIFIED][/yellow]"
                console.print(f"    {status_label} {rel_p}")

    # 3. Artifacts Summary Table
    console.print("\n[bold blue]📦 Context Artifacts[/bold blue] [dim](.codigest/)[/dim]")
    
    artifacts = [
        ("snapshot.txt", "Full Context Snapshot", "cdg scan"),
        ("changes.diff", "Incremental Diff", "cdg diff"),
        ("semdiff.txt", "Semantic AST Diff", "cdg semdiff"),
        ("digest.txt", "Architecture Digest", "cdg digest"),
    ]

    table = Table(box=None, header_style="bold cyan", padding=(0, 2))
    table.add_column("Artifact", style="bold white")
    table.add_column("Description", style="dim")
    table.add_column("Status")
    table.add_column("Size", justify="right")
    table.add_column("Tokens", justify="right", style="cyan")

    for filename, desc, cmd_hint in artifacts:
        art_path = codigest_dir / filename
        if art_path.exists() and art_path.is_file():
            stat = art_path.stat()
            size_str = decimal(stat.st_size)
            try:
                text_content = art_path.read_text(encoding="utf-8", errors="replace")
                tokens = tokenizer.estimate_tokens(text_content)
                tokens_str = f"~{tokens:,}"
            except Exception:
                tokens_str = "N/A"
            status_str = "[bold green]Ready[/bold green]"
        else:
            size_str = "-"
            tokens_str = "-"
            status_str = f"[dim]Missing ({cmd_hint})[/dim]"

        table.add_row(filename, desc, status_str, size_str, tokens_str)

    console.print(table)

    # 4. Configuration Summary
    console.print("\n[bold blue]⚙️ Configuration[/bold blue] [dim](.codigest/config.toml)[/dim]")
    config_file = codigest_dir / "config.toml"
    if config_file.exists():
        console.print("  • [bold]Config File:[/bold] [green]Found[/green]")
    else:
        console.print("  • [bold]Config File:[/bold] [yellow]Default settings (no config.toml)[/yellow]")

    if ctx.config_extensions:
        exts_str = ", ".join(sorted(ctx.config_extensions))
        console.print(f"  • [bold]Filter Extensions:[/bold] [cyan]{exts_str}[/cyan]")
    else:
        console.print("  • [bold]Filter Extensions:[/bold] [dim]All non-ignored extensions[/dim]")

    if ctx.config_ignores:
        console.print(f"  • [bold]Extra Ignores:[/bold] {len(ctx.config_ignores)} custom patterns")
