from pathlib import Path

import typer

from ..options import CopyOption, StdoutOption, ResolveOption, MessageOption, OutputOption

lazy from ..core import structure, prompts, outline, tags, tokenizer, common, cache
lazy from ..core.output import OutputHandler, spinner

app = typer.Typer()

@app.callback(invoke_without_command=True)
def handle(
    targets: list[Path] = typer.Argument(
        None, help="Files or directories to outline (default: the project)", exists=True, resolve_path=True
    ),
    copy: CopyOption = None,
    output: OutputOption = None,
    stdout: StdoutOption = False,
    message: MessageOption = "",
    resolve: ResolveOption = False,
) -> None:
    """
    Outline Python and MATLAB files: numbered signatures and docstrings, no bodies.
    """
    handler = OutputHandler(stdout=stdout, copy=copy, output=output)
    log_console = handler.log_console

    ctx = common.get_context(targets)
    root_path = ctx.root_path

    prompt_engine = prompts.get_engine(root_path)

    with spinner(log_console, "[bold blue]Digesting...[/bold blue]"):
        files = ctx.get_target_files(targets=targets, resolve_deps=resolve)
        tree_str = structure.generate_ascii_tree(files, root_path)

        content_cache = cache.ContentCache(root_path)
        summary_blocks = []
        unreadable = []

        for file_path in files:
            if file_path.suffix not in outline.SUFFIXES:
                continue
            try:
                rel_path = file_path.relative_to(root_path).as_posix()
            except ValueError:
                rel_path = f"[EXTERNAL]/{file_path.name}"

            summary = content_cache.get_summary(rel_path, blob_hash=None, file_path=file_path)
            if summary is None:
                try:
                    content = file_path.read_text(encoding="utf-8", errors="replace")
                except OSError:
                    unreadable.append(rel_path)
                    continue
                summary = outline.summarize(file_path.suffix, content)
                if summary and (blob_hash := cache.get_file_blob_hash(file_path)):
                    content_cache.set_summary(rel_path, blob_hash, summary, file_path=file_path)

            if summary:
                summary_blocks.append(tags.file(rel_path, summary))

        content_cache.save()

        digest_content = prompt_engine.render(
            "digest",
            project_name=root_path.name,
            tree_structure=tree_str,
            digest_content="\n".join(summary_blocks),
            instruction=message,
        )

    if unreadable:
        log_console.print(f"[yellow]Could not read:[/yellow] {', '.join(unreadable)}", highlight=False)
    token_count = tokenizer.estimate_tokens(digest_content)
    log_console.print(f"[bold]Digest[/bold] {len(summary_blocks)} files, ~{token_count:,} tokens")

    handler.handle_output(digest_content, item_name="digest")
