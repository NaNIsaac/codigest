import typer
import pyperclip
from pathlib import Path
from rich.console import Console
from rich.progress import Progress, SpinnerColumn, TextColumn

from ..core import structure, prompts, semdiff, tags, tokenizer, common, cache

app = typer.Typer()
console = Console()
err_console = Console(stderr=True)

@app.callback(invoke_without_command=True)
def handle(
    target: Path = typer.Argument(Path.cwd(), help="Target directory"),
    copy: bool = typer.Option(True, help="Auto-copy to clipboard"),
    save: bool = typer.Option(True, help="Save to .codigest/digest.txt"),
    stdout: bool = typer.Option(False, "-s", "--stdout", help="Print output to terminal (stdout) instead of file"),
    message: str = typer.Option("", "--message", "-m", help="Add specific instruction"),
    # [추가]
    resolve: bool = typer.Option(False, "-r", "--resolve", help="Recursively resolve imports"),
):
    """
    [Architectural View] Summarizes the codebase structure (Classes/Functions only).
    """
    log_console = err_console if stdout else console

    # [1] Context Setup
    ctx = common.get_context(target)
    root_path = ctx.root_path
    
    prompt_engine = prompts.get_engine(root_path)

    with Progress(
        SpinnerColumn(),
        TextColumn("[bold blue]Digesting Architecture...[/bold blue]"),
        transient=True,
        console=log_console
    ) as progress:
        
        task = progress.add_task("digest", total=None)
        
        # [2] File Discovery via Context
        files = ctx.get_target_files(ignore_config=False, resolve_deps=resolve)
        
        tree_str = structure.generate_ascii_tree(files, root_path)

        content_cache = cache.ContentCache(root_path)
        summary_blocks = []

        for file_path in files:
            if file_path.suffix in (".py", ".pyi"):
                try:
                    rel_path = file_path.relative_to(root_path).as_posix()
                except ValueError:
                    rel_path = f"[EXTERNAL]/{file_path.name}"

                summary = content_cache.get_summary(rel_path, blob_hash=None, file_path=file_path)

                if summary is None:
                    try:
                        blob_hash = cache.get_file_blob_hash(file_path)
                        content = file_path.read_text(encoding="utf-8")
                        summary = semdiff.summarize(content)
                        if summary and blob_hash:
                            content_cache.set_summary(rel_path, blob_hash, summary, file_path=file_path)
                    except Exception:
                        continue

                if summary:
                    block = tags.file(rel_path, summary)
                    summary_blocks.append(block)

        content_cache.save()

        summary_blob = "\n".join(summary_blocks)
        
        try:
            digest_content = prompt_engine.render(
                "digest",
                project_name=root_path.name,
                tree_structure=tree_str,
                digest_content=summary_blob,
                instruction=message
            )
        except Exception as e:
            log_console.print(f"[red]❌ Rendering Failed:[/red] {e}")
            raise typer.Exit(1)
            
        progress.update(task, completed=100)

    token_count = tokenizer.estimate_tokens(digest_content)
    log_console.print(f"[bold green]Digest Generated![/bold green] ([bold cyan]~{token_count:,} Tokens[/bold cyan])")
    
    if copy:
        try:
            pyperclip.copy(digest_content)
            log_console.print("[dim]📋 Copied to clipboard[/dim]")
        except Exception:
            log_console.print("[dim]⚠️ Clipboard unavailable (skipped copy)[/dim]")
    
    if stdout:
        print(digest_content)
    elif save:
        out_path = root_path / ".codigest" / "digest.txt"
        out_path.parent.mkdir(exist_ok=True)
        out_path.write_text(digest_content, encoding="utf-8")
        log_console.print(f"[dim]💾 Saved to {out_path}[/dim]")