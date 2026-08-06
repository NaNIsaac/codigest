import sys
import typer
from pathlib import Path
from rich.progress import Progress, SpinnerColumn, TextColumn
from rich.panel import Panel
from rich.filesize import decimal

from ..options import CopyOption, StdoutOption, ResolveOption, MessageOption, AllOption
from ..core import structure, tags, prompts, processor, shadow, tokenizer, common, cache
from ..core.output import OutputHandler

app = typer.Typer()

@app.callback(invoke_without_command=True)
def handle(
    targets: list[Path] = typer.Argument(
        None, 
        help="Specific files or directories to scan (Scope)",
        exists=True,
        resolve_path=True
    ),
    output: str = typer.Option("snapshot.txt", "-o", "--output", help="Output filename inside .codigest/"),
    copy: CopyOption = True,
    stdout: StdoutOption = False,
    all: AllOption = False,
    message: MessageOption = "",
    line_numbers: bool = typer.Option(False, "--lines", "-l", help="Add line numbers to code blocks"),
    yes: bool = typer.Option(False, "-y", "--yes", help="Skip confirmation prompt"),
    resolve: ResolveOption = False,
):
    """
    Scans the codebase. 
    If TARGETS provided, only scans those paths within the project.
    """
    handler = OutputHandler(stdout=stdout, copy=copy)
    log_console = handler.log_console

    # [1] Context Setup (Centralized)
    ctx = common.get_context(targets)
    root_path = ctx.root_path

    # Init check
    artifact_dir = root_path / ".codigest"
    if not artifact_dir.exists():
        log_console.print(f"[yellow][Warning] .codigest directory missing in {root_path.name}. Running init...[/yellow]")
        try:
            artifact_dir.mkdir(exist_ok=True)
        except PermissionError:
            log_console.print(f"[red][Error] Cannot create .codigest at {root_path}[/red]")
            raise typer.Exit(1)

    output_path = artifact_dir / output
    prompt_engine = prompts.get_engine(root_path)
    anchor = shadow.ContextAnchor(root_path)

    # [2] File Discovery via Context
    with Progress(
        SpinnerColumn(),
        TextColumn("[bold blue]Scanning...[/bold blue]"),
        transient=True,
        console=log_console
    ) as progress:
        task = progress.add_task("scanning", total=None)
        
        # ★ All logic delegated to common.py
        files = ctx.get_target_files(
            targets=targets, 
            ignore_config=all, 
            resolve_deps=resolve
        )
        
        progress.update(task, completed=100)

    # [3] Pre-flight Check
    total_files = len(files)
    total_size = sum(f.stat().st_size for f in files)
    est_tokens = int(total_size / 4) 

    log_console.print(Panel(f"""[bold]Scan Plan[/bold]
  Target: [cyan]{root_path}[/cyan]
  Scope: {total_files} files
  Est. Size: {decimal(total_size)}
  Est. Tokens: ~{est_tokens:,}""", expand=False))

    TOKEN_THRESHOLD = 30000   
    FILE_COUNT_THRESHOLD = 100
    is_large_context = est_tokens > TOKEN_THRESHOLD or total_files > FILE_COUNT_THRESHOLD

    if yes:
        pass 
    elif is_large_context:
        log_console.print(f"[yellow][Warning] Large context detected (> {TOKEN_THRESHOLD:,} tokens or > {FILE_COUNT_THRESHOLD} files).[/yellow]")
        if not typer.confirm("Proceed with digestion?"):
            log_console.print("[red]Aborted.[/red]")
            raise typer.Exit()
    else:
        log_console.print("[dim]Small context detected. Automatically proceeding...[/dim]")

    # [4] Execution
    with Progress(
        SpinnerColumn(),
        TextColumn("[bold blue]Generating Snapshot...[/bold blue]"),
        transient=True,
        console=log_console
    ) as progress:
        
        if anchor.has_history():
            try:
                diff_content = anchor.get_changes(files)
                if diff_content.strip():
                    pre_diff_path = artifact_dir / "previous_changes.diff"
                    pre_diff_path.write_text(diff_content, encoding="utf-8")
            except Exception:
                pass

        tree_str = structure.generate_ascii_tree(files, root_path)

        content_cache = cache.ContentCache(root_path)
        file_blocks = []

        for file_path in files:
            try:
                rel_path = file_path.relative_to(root_path).as_posix()
            except ValueError:
                rel_path = f"[EXTERNAL]/{file_path.name}"

            cached_block = content_cache.get_file_block(
                rel_path=rel_path, 
                line_numbers=line_numbers, 
                file_path=file_path
            )

            if cached_block is not None:
                file_blocks.append(cached_block)
            else:
                try:
                    blob_hash = cache.get_file_blob_hash(file_path)
                    content = processor.read_file_content(file_path, add_line_numbers=line_numbers)
                    block = tags.file(rel_path, content)
                    file_blocks.append(block)
                    content_cache.set_file_block(
                        rel_path=rel_path, 
                        blob_hash=blob_hash, 
                        line_numbers=line_numbers, 
                        block=block, 
                        file_path=file_path
                    )
                except Exception:
                    continue

        content_cache.save()

        source_code_blob = "\n\n".join(file_blocks)

        try:
            snapshot_content = prompt_engine.render(
                "snapshot",
                project_name=root_path.name,
                tree_structure=tree_str,
                source_code=source_code_blob,
                instruction=message
            )
        except Exception as e:
            log_console.print(f"[red][Error] Template Rendering Failed:[/red] {e}")
            raise typer.Exit(1)

    try:
        anchor.update(files)
    except Exception as e:
        log_console.print(f"[yellow][Warning] Failed to update context anchor: {e}[/yellow]")

    def _save():
        try:
            output_path.write_text(snapshot_content, encoding="utf-8")
            final_token_count = tokenizer.estimate_tokens(snapshot_content)

            log_console.print("[bold green]Snapshot Saved![/bold green]")
            log_console.print(f"  Path: [underline]{output_path}[/underline]")
            log_console.print(f"  Final Tokens: [bold cyan]~{final_token_count:,}[/bold cyan]")
            
            if anchor.has_history():
                pre_diff_path = artifact_dir / "previous_changes.diff"
                if pre_diff_path.exists() and pre_diff_path.stat().st_size > 0:
                    log_console.print(f"  [dim]Changes before this scan saved to: {pre_diff_path.name}[/dim]")

        except Exception as e:
            log_console.print(f"[bold red][Error] Save Failed:[/bold red] {e}")
            raise typer.Exit(1)

    handler.handle_output(snapshot_content, save_action=_save)