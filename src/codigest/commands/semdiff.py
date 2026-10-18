from pathlib import Path

import typer

from ..options import CopyOption, StdoutOption, ResolveOption, MessageOption, OutputOption

lazy from rich.progress import Progress, SpinnerColumn, TextColumn
lazy from ..core import prompts, shadow, semdiff, tags, tokenizer, common
lazy from ..core.output import OutputHandler

app = typer.Typer()

@app.callback(invoke_without_command=True)
def handle(
    target: Path = typer.Argument(Path("."), help="Target directory"),
    copy: CopyOption = True,
    output: OutputOption = None,
    stdout: StdoutOption = False,
    message: MessageOption = "",
    resolve: ResolveOption = False,
) -> None:
    """
    Show which Python functions and classes changed since the baseline.
    """
    handler = OutputHandler(stdout=stdout, copy=copy, output=output)
    log_console = handler.log_console

    # [1] Context Setup
    ctx = common.get_context(target)
    root_path = ctx.root_path

    anchor = shadow.ContextAnchor(root_path)

    last_update = anchor.get_last_update_time()
    if last_update == "Never":
        log_console.print("[yellow]No baseline found. Run [bold]cdg mark[/bold] first.[/yellow]")
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

        target_files = [p for p in changed_files_paths if p.suffix in (".py", ".pyi")]

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
            except Exception:
                new_code = ""

            # Read Old Content (From Anchor)
            # Note: Anchor only stores files inside root. External files will likely return empty old_code.
            old_code = anchor.read_anchor_file(rel_path) if rel_path else ""

            changes = semdiff.compare(old_code, new_code)

            if changes:
                change_lines = []
                for ch in changes:
                    match ch.change_type:
                        case "ADDED": icon = "[ADDED]    "
                        case "REMOVED": icon = "[REMOVED]  "
                        case "MODIFIED": icon = "[SIGNATURE]"
                        case _: icon = "[LOGIC]    "

                    detail = f" :: {ch.details}" if ch.details else ""
                    line = f"{icon} {ch.symbol.type} {ch.symbol.name}{ch.symbol.signature}{detail}"
                    change_lines.append(line)

                # File Status Tag
                file_status = " (DELETED)" if not new_code else " (NEW)" if not old_code else ""

                raw_body = "\n".join(change_lines)

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
            context_message=f"since cdg mark ({last_update})",
            semdiff_content=report_content,
            instruction=message
        )
    except Exception as e:
        log_console.print(f"[red]Template Error:[/red] {e}")
        raise typer.Exit(1) from e

    token_count = tokenizer.estimate_tokens(final_output)
    log_console.print(f"[bold green]SemDiff Generated![/bold green] ([bold cyan]~{token_count:,} Tokens[/bold cyan])")

    handler.handle_output(final_output, item_name="semdiff")