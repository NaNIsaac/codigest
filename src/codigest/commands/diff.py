import typer
from pathlib import Path
from rich.progress import Progress, SpinnerColumn, TextColumn

from ..options import CopyOption, StdoutOption, ResolveOption, MessageOption
from ..core import prompts, shadow, tags, common
from ..core.output import OutputHandler

app = typer.Typer()

@app.callback(invoke_without_command=True)
def handle(
    target: Path = typer.Argument(
        Path.cwd(), 
        help="Target directory to check diff",
        exists=True,
        file_okay=False,
        dir_okay=True,
        resolve_path=True
    ),
    copy: CopyOption = True,
    save: bool = typer.Option(True, help="Save to .codigest/changes.diff"),
    stdout: StdoutOption = False,
    message: MessageOption = "",
    resolve: ResolveOption = False,
):
    """
    [Context Update] Shows changes since the last 'codigest scan'.
    Useful for updating LLM context without re-uploading everything.
    """
    handler = OutputHandler(stdout=stdout, copy=copy)
    log_console = handler.log_console

    # [1] Context Setup
    ctx = common.get_context(target)
    root_path = ctx.root_path
    
    anchor = shadow.ContextAnchor(root_path)

    # Check Baseline
    last_update = anchor.get_last_update_time()
    if last_update == "Never":
        log_console.print("[yellow]⚠️  No scan history found.[/yellow]")
        log_console.print("   Run [bold cyan]cdg scan[/bold cyan] first to establish a baseline.")
        raise typer.Exit(1)

    log_console.print(f"[dim]Checking changes since last scan ({last_update})...[/dim]")

    # [2] Calculate Diff via Context
    with Progress(
        SpinnerColumn(),
        TextColumn("[bold blue]Analyzing changes...[/bold blue]"),
        transient=True,
        console=log_console
    ) as progress:
        task = progress.add_task("diff", total=None)

        # ★ Use common context to get files (handles config & resolve)
        current_files = ctx.get_target_files(resolve_deps=resolve)

        # Compare Working Tree vs Anchor
        diff_content = anchor.get_changes(current_files)

        progress.update(task, completed=100)

    if not diff_content.strip():
        log_console.print("[green]No changes detected since last scan.[/green]")
        return

    # [3] Render
    prompt_engine = prompts.get_engine(root_path)
    try:
        formatted_diff = prompt_engine.render(
            "diff",
            project_name=root_path.name,
            context_message=f"Changes since last scan ({last_update})",
            diff_content=diff_content,
            instruction=message
        )
    except Exception:
        # Fallback if template fails (using safe escaping via tags)
        safe_content = tags.escape_xml_value(diff_content)
        formatted_diff = f"<diff>\n{safe_content}\n</diff>"

    # [4] Output
    log_console.print(f"[bold green]Changes Detected![/bold green] ({len(formatted_diff)} chars)")
    
    def _save():
        if save:
            out_path = root_path / ".codigest" / "changes.diff"
            out_path.parent.mkdir(exist_ok=True)
            out_path.write_text(formatted_diff, encoding="utf-8")
            log_console.print(f"[dim]Saved to {out_path}[/dim]")

    handler.handle_output(formatted_diff, save_action=_save, item_name="diff")