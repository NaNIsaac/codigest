import typer
import pyperclip
from pathlib import Path
from rich.console import Console
from rich.progress import Progress, SpinnerColumn, TextColumn

from ..core import scanner, prompts, shadow
from .scan import _load_config_filters 

app = typer.Typer()
console = Console()

@app.callback(invoke_without_command=True)
def handle(
    # [변경 1] target 인자 추가
    target: Path = typer.Argument(
        Path.cwd(), 
        help="Target directory to check diff",
        exists=True,
        file_okay=False,
        dir_okay=True,
        resolve_path=True
    ),
    copy: bool = typer.Option(True, help="Auto-copy to clipboard"),
    save: bool = typer.Option(True, help="Save to .codigest/changes.diff"),
    message: str = typer.Option("", "--message", "-m", help="Add specific instruction context"),
):
    """
    [Context Update] Shows changes since the last 'codigest scan'.
    Useful for updating LLM context without re-uploading everything.
    """
    root_path = target
    anchor = shadow.ContextAnchor(root_path)
    
    # 1. Check Baseline Existence
    last_update = anchor.get_last_update_time()
    if last_update == "Never":
        console.print("[yellow]⚠️  No scan history found.[/yellow]")
        console.print("   Run [bold cyan]codigest scan[/bold cyan] first to establish a baseline.")
        raise typer.Exit(1)

    console.print(f"[dim]Checking changes since last scan ({last_update})...[/dim]")

    # 2. Calculate Diff
    with Progress(
        SpinnerColumn(),
        TextColumn("[bold blue]Analyzing changes...[/bold blue]"),
        transient=True,
        console=console
    ) as progress:
        task = progress.add_task("diff", total=None)
        
        # Load same filters as scan
        extensions, extra_ignores = _load_config_filters(root_path)
        
        # Get current valid files (Working Tree)
        current_files = scanner.scan_project(root_path, extensions, extra_ignores)
        
        # Compare Working Tree vs Anchor
        diff_content = anchor.get_changes(current_files)
        
        progress.update(task, completed=100)

    if not diff_content.strip():
        console.print("[green]✨ No changes detected since last scan.[/green]")
        return

    # 3. Render
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
        # Fallback if template fails
        formatted_diff = f"<diff>\n{diff_content}\n</diff>"

    # 4. Output
    console.print(f"[bold green]✔ Changes Detected![/bold green] ({len(formatted_diff)} chars)")
    
    if copy:
        pyperclip.copy(formatted_diff)
        console.print("[dim]📋 Copied to clipboard[/dim]")

    if save:
        out_path = root_path / ".codigest" / "changes.diff"
        out_path.parent.mkdir(exist_ok=True)
        out_path.write_text(formatted_diff, encoding="utf-8")
        console.print(f"[dim]💾 Saved to {out_path}[/dim]")
