import typer
from pathlib import Path
from rich.progress import Progress, SpinnerColumn, TextColumn

from ..options import CopyOption, StdoutOption, ResolveOption, MessageOption
from ..core import prompts, shadow, semdiff, tags, tokenizer, common
from ..core.output import OutputHandler

app = typer.Typer()

@app.callback(invoke_without_command=True)
def handle(
    target: Path = typer.Argument(Path.cwd(), help="Target directory"),
    copy: CopyOption = True,
    save: bool = typer.Option(True, help="Save to .codigest/semdiff.txt"),
    stdout: StdoutOption = False,
    message: MessageOption = "",
    resolve: ResolveOption = False,
):
    """
    [Advanced] Generates a Semantic Diff (AST-based) report.
    Shows ADDED/REMOVED/MODIFIED functions & classes instead of raw text lines.
    """
    handler = OutputHandler(stdout=stdout, copy=copy)
    log_console = handler.log_console

    # [1] Context Setup
    ctx = common.get_context(target)
    root_path = ctx.root_path
    
    anchor = shadow.ContextAnchor(root_path)

    last_update = anchor.get_last_update_time()
    if last_update == "Never":
        log_console.print("[yellow]⚠️  No scan history found. Run [bold]cdg scan[/bold] first.[/yellow]")
        raise typer.Exit(1)

    log_console.print(f"[dim]Analyzing structural changes since ({last_update})...[/dim]")

    reports = []

    with Progress(
        SpinnerColumn(),
        TextColumn("[bold blue]Parsing AST...[/bold blue]"),
        transient=True,
        console=log_console
    ) as progress:
        task = progress.add_task("semdiff", total=None)

        # [2] Get Files via Context
        current_files = ctx.get_target_files(resolve_deps=resolve)

        # Only check files that have text changes first (Optimization)
        changed_files_paths = anchor.get_changed_files(list(current_files))

        target_files = []
        for p in changed_files_paths:
            if p.suffix in (".py", ".pyi"):
                target_files.append(p)

        # [3] Compare Each File
        for file_path in target_files:
            # Handle Path String (Support External Files)
            try:
                rel_path = file_path.relative_to(root_path)
                rel_path_str = rel_path.as_posix()
            except ValueError:
                # External files tracked via resolve logic
                rel_path = None # Anchor might not support reading external files yet
                rel_path_str = f"[EXTERNAL]/{file_path.name}"
            
            # Read New Content
            try:
                new_code = file_path.read_text(encoding="utf-8") if file_path.exists() else ""
            except:
                new_code = ""
            
            # Read Old Content (From Anchor)
            # Note: Anchor only stores files inside root. External files will likely return empty old_code.
            if rel_path:
                old_code = anchor.read_anchor_file(rel_path)
            else:
                old_code = "" 

            changes = semdiff.compare(old_code, new_code)

            if changes:
                change_lines = []
                for ch in changes:
                    if ch.change_type == "ADDED": icon = "➕ [ADDED]   "
                    elif ch.change_type == "REMOVED": icon = "➖ [REMOVED] "
                    elif ch.change_type == "MODIFIED": icon = "⚠️ [SIGNATURE]"
                    else: icon = "✏️ [LOGIC]   "
                    
                    detail = f" :: {ch.details}" if ch.details else ""
                    line = f"{icon} {ch.symbol.type} {ch.symbol.name}{ch.symbol.signature}{detail}"
                    change_lines.append(line)

                # File Status Tag
                file_status = ""
                if not new_code: file_status = " (DELETED)"
                elif not old_code: file_status = " (NEW)"

                raw_body = chr(10).join(change_lines)
                
                # Use unified tags factory
                block = tags.file(rel_path_str, raw_body, status=file_status)
                reports.append(block)

        progress.update(task, completed=100)

    if not reports:
        log_console.print("[green]No structural (AST) changes detected.[/green]")
        return

    # [4] Render
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
        log_console.print(f"[red]Template Error:[/red] {e}")
        raise typer.Exit(1)

    token_count = tokenizer.estimate_tokens(final_output)
    log_console.print(f"[bold green]SemDiff Generated![/bold green] ([bold cyan]~{token_count:,} Tokens[/bold cyan])")
    
    def _save():
        if save:
            out_path = root_path / ".codigest" / "semdiff.txt"
            out_path.parent.mkdir(exist_ok=True)
            out_path.write_text(final_output, encoding="utf-8")
            log_console.print(f"[dim]Saved to {out_path}[/dim]")

    handler.handle_output(final_output, save_action=_save, item_name="semdiff")