from pathlib import Path

import typer

from ..options import CopyOption, StdoutOption, ResolveOption, MessageOption, OutputOption

lazy from ..core import prompts, shadow, tags, common
lazy from ..core.redact import redact_diff
lazy from ..core.output import OutputHandler, spinner

app = typer.Typer()


@app.callback(invoke_without_command=True)
def handle(
    target: Path = typer.Argument(
        Path("."),
        help="Target directory to check diff",
        exists=True,
        file_okay=False,
        dir_okay=True,
        resolve_path=True,
    ),
    copy: CopyOption = None,
    output: OutputOption = None,
    stdout: StdoutOption = False,
    message: MessageOption = "",
    resolve: ResolveOption = False,
) -> None:
    """
    Show the text diff since the baseline (`cdg mark`).
    """
    handler = OutputHandler(stdout=stdout, copy=copy, output=output)
    log_console = handler.log_console

    # [1] Context Setup
    ctx = common.get_context(target)
    root_path = ctx.root_path

    anchor = shadow.ContextAnchor(root_path)

    # Check Baseline
    last_update = anchor.get_last_update_time()
    if last_update == "Never":
        log_console.print("[yellow]No baseline found.[/yellow]")
        log_console.print("   Run [bold cyan]cdg mark[/bold cyan] first to set a baseline.")
        raise typer.Exit(1)

    log_console.print(f"[dim]Checking changes since last mark ({last_update})...[/dim]")

    # [2] Calculate Diff via Context
    with spinner(log_console, "[bold blue]Analyzing changes...[/bold blue]"):
        current_files = ctx.get_target_files(resolve_deps=resolve)

        # Compare Working Tree vs Anchor
        diff_content = redact_diff(anchor.get_changes(current_files))

    if not diff_content.strip():
        log_console.print("[green]No changes since the last mark.[/green]")
        return

    # [3] Render
    prompt_engine = prompts.get_engine(root_path)
    try:
        formatted_diff = prompt_engine.render(
            "diff",
            project_name=root_path.name,
            context_message=f"since cdg mark ({last_update})",
            diff_content=diff_content,
            instruction=message,
        )
    except Exception:
        # Fallback if template fails (using safe escaping via tags)
        safe_content = tags.escape_xml_value(diff_content)
        formatted_diff = f"<diff>\n{safe_content}\n</diff>"

    # [4] Output
    log_console.print(f"[bold green]Changes Detected![/bold green] ({len(formatted_diff)} chars)")

    handler.handle_output(formatted_diff, item_name="diff")
