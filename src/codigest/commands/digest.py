from pathlib import Path

import typer

from ..options import CopyOption, StdoutOption, ResolveOption, MessageOption, OutputOption

lazy from ..core import structure, prompts, semdiff, tags, tokenizer, common, cache
lazy from ..core.output import OutputHandler, spinner

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
    Show Python signatures and docstrings without function bodies.
    """
    handler = OutputHandler(stdout=stdout, copy=copy, output=output)
    log_console = handler.log_console

    # [1] Context Setup
    ctx = common.get_context(target)
    root_path = ctx.root_path

    prompt_engine = prompts.get_engine(root_path)

    with spinner(log_console, "[bold blue]Digesting Architecture...[/bold blue]"):


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
            log_console.print(f"[red]Rendering failed:[/red] {e}")
            raise typer.Exit(1) from e


    token_count = tokenizer.estimate_tokens(digest_content)
    log_console.print(f"[bold green]Digest Generated![/bold green] ([bold cyan]~{token_count:,} Tokens[/bold cyan])")

    handler.handle_output(digest_content, item_name="digest")