import typer
import pyperclip
from pathlib import Path
from rich.console import Console
from rich.progress import Progress, SpinnerColumn, TextColumn

from ..core import scanner, prompts, shadow, semdiff, tags, tokenizer

# Reuse config loader
from .scan import _load_config_filters 

app = typer.Typer()
console = Console()

@app.callback(invoke_without_command=True)
def handle(
    target: Path = typer.Argument(Path.cwd(), help="Target directory"),
    copy: bool = typer.Option(True, help="Auto-copy to clipboard"),
    save: bool = typer.Option(True, help="Save to .codigest/semdiff.xml"),
    message: str = typer.Option("", "--message", "-m", help="Add specific instruction"),
):
    """
    [Advanced] Generates a Semantic Diff (AST-based) report.
    Shows ADDED/REMOVED/MODIFIED functions & classes instead of raw text lines.
    """
    root_path = target.resolve()
    anchor = shadow.ContextAnchor(root_path)

    last_update = anchor.get_last_update_time()
    if last_update == "Never":
        console.print("[yellow]⚠️  No scan history found. Run [bold]codigest scan[/bold] first.[/yellow]")
        raise typer.Exit(1)

    console.print(f"[dim]Analyzing structural changes since ({last_update})...[/dim]")

    reports = []

    with Progress(
        SpinnerColumn(),
        TextColumn("[bold blue]Parsing AST...[/bold blue]"),
        transient=True,
        console=console
    ) as progress:
        task = progress.add_task("semdiff", total=None)

        # 1. Scan files
        extensions, extra_ignores = _load_config_filters(root_path)
        current_files = scanner.scan_project(root_path, extensions, extra_ignores)

        changed_files_paths = anchor.get_changed_files(list(current_files))

        target_files = []
        for p in changed_files_paths:
            if p.suffix in (".py", ".pyi"):
                target_files.append(p)

        # 2. Compare Each File
        for file_path in target_files:
            rel_path = file_path.relative_to(root_path)
            
            # Read Content (Handle Missing Files for Deletion)
            try:
                new_code = file_path.read_text(encoding="utf-8") if file_path.exists() else ""
            except:
                new_code = ""
                
            old_code = anchor.read_anchor_file(rel_path)
            
            # Case: File Added (No old code) -> All symbols ADDED
            # Case: File Deleted (No new code) -> All symbols REMOVED
            # Case: Modified -> Semantic Compare

            changes = semdiff.compare(old_code, new_code)

            if changes:
                change_lines = []
                for ch in changes:
                    if ch.change_type == "ADDED":
                        icon = "➕ [ADDED]   "
                    elif ch.change_type == "REMOVED":
                        icon = "➖ [REMOVED] "
                    elif ch.change_type == "MODIFIED":
                        icon = "⚠️ [SIGNATURE]" # 중요: 시그니처 변경
                    else:
                        icon = "✏️ [LOGIC]   "
                        
                    detail = f" :: {ch.details}" if ch.details else ""
                    line = f"{icon} {ch.symbol.type} {ch.symbol.name}{ch.symbol.signature}{detail}"
                    change_lines.append(line)

                # File Deleted Tagging
                file_status = ""
                if not new_code:
                    file_status = " (DELETED)"
                elif not old_code:
                    file_status = " (NEW)"

                raw_body = chr(10).join(change_lines)
                block = tags.file(rel_path.as_posix(), raw_body, status=file_status)
                reports.append(block)

        progress.update(task, completed=100)

    if not reports:
        console.print("[green]✨ No structural (AST) changes detected.[/green]")
        return

    # Render
    report_content = "\n".join(reports)
    prompt_engine = prompts.get_engine(root_path)

    try:
        final_output = prompt_engine.render(
            "semdiff",
            project_name=root_path.name,
            context_message=f"Structural changes since {last_update}",
            semdiff_content=report_content,
            instruction=message
        )
    except Exception as e:
        console.print(f"[red]❌ Template Error:[/red] {e}")
        raise typer.Exit(1)

    token_count = tokenizer.estimate_tokens(final_output)
    console.print(f"[bold green]✔ SemDiff Generated![/bold green] ([bold cyan]~{token_count:,} Tokens[/bold cyan])")
    
    if copy:
        pyperclip.copy(final_output)
        console.print("[dim]📋 Copied to clipboard[/dim]")
    
    if save:
        out_path = root_path / ".codigest" / "semdiff.xml"
        out_path.parent.mkdir(exist_ok=True)
        out_path.write_text(final_output, encoding="utf-8")
        console.print(f"[dim]💾 Saved to {out_path}[/dim]")
