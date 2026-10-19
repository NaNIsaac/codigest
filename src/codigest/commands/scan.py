import re
import sys
from pathlib import Path

import typer

from ..options import CopyOption, StdoutOption, ResolveOption, MessageOption, AllOption, OutputOption

lazy from rich.filesize import decimal
lazy from ..core import structure, tags, prompts, processor, tokenizer, common, cache
lazy from ..core.output import OutputHandler, spinner

app = typer.Typer()

# `src/app.py:40-60` or `src/app.py:42`; the last colon, so `C:\x.py:3` still works.
_RANGE = re.compile(r"^(?P<path>.+):(?P<lo>\d+)(?:-(?P<hi>\d+))?$")


def _parse_targets(raw: list[str]) -> tuple[list[Path], dict[Path, list[tuple[int, int | None]]]]:
    paths, ranges = [], {}
    for item in raw:
        path, span = Path(item), None
        # Range syntax wins: on NTFS `a.py:12` names an alternate data stream of a.py,
        # which the path checks below would not reject.
        if (m := _RANGE.match(item)) and Path(m["path"]).is_file():
            path = Path(m["path"])
            span = (int(m["lo"]), int(m["hi"]) if m["hi"] else None)
        if not path.exists():
            hint = " (a line range is FILE:START-END or FILE:LINE)" if ":" in item[2:] else ""
            raise typer.BadParameter(f"{item} does not exist{hint}", param_hint="targets")
        path = path.resolve()
        if span:
            ranges.setdefault(path, []).append(span)
        if path not in paths:
            paths.append(path)
    return paths, ranges


@app.callback(invoke_without_command=True)
def handle(
    raw_targets: list[str] = typer.Argument(
        None,
        metavar="[targets]...",
        help="Files or directories to share; FILE:40-60 or FILE:42 shares only those lines",
        show_default=False,
    ),
    output: OutputOption = None,
    copy: CopyOption = None,
    stdout: StdoutOption = False,
    all: AllOption = False,
    message: MessageOption = "",
    line_numbers: bool = typer.Option(False, "--lines", "-l", help="Add line numbers to code blocks"),
    yes: bool = typer.Option(False, "-y", "--yes", help="Skip confirmation prompt"),
    resolve: ResolveOption = False,
) -> None:
    """
    Bundle the whole project, or only the given paths, as one block.
    """
    handler = OutputHandler(stdout=stdout, copy=copy, output=output)
    log_console = handler.log_console
    targets, ranges = _parse_targets(raw_targets or [])

    # [1] Context Setup (Centralized)
    ctx = common.get_context(targets)
    root_path = ctx.root_path

    prompt_engine = prompts.get_engine(root_path)

    # [2] File Discovery via Context
    with spinner(log_console, "[bold blue]Scanning...[/bold blue]"):

        
        files = ctx.get_target_files(
            targets=targets,
            ignore_config=all,
            resolve_deps=resolve
        )


    # [3] Pre-flight Check
    total_files = len(files)
    # A range of a big file is small: count what will be shared, not the whole file.
    total_size = sum(
        sum(len(processor.read_line_range(f, lo, hi)[0]) for lo, hi in ranges[f])
        if f in ranges else f.stat().st_size
        for f in files
    )
    est_tokens = int(total_size / 4)

    if ctx.withheld:
        names = ", ".join(p.relative_to(root_path).as_posix() for p in ctx.withheld)
        log_console.print(f"[yellow]Withheld (may contain secrets):[/yellow] {names}", highlight=False)

    TOKEN_THRESHOLD = 30000
    FILE_COUNT_THRESHOLD = 100
    is_large_context = est_tokens > TOKEN_THRESHOLD or total_files > FILE_COUNT_THRESHOLD

    if yes:
        pass
    elif is_large_context:
        log_console.print(
            f"[yellow]Large:[/yellow] {total_files} files, {decimal(total_size)}, about {est_tokens:,} tokens "
            f"(asks above {TOKEN_THRESHOLD:,} tokens or {FILE_COUNT_THRESHOLD} files; -y skips this)"
        )
        hint = "Share less by naming paths (cdg scan src/), or cdg digest for signatures only."
        if not sys.stdin.isatty():
            # Nobody can answer the question: say how to go on instead of a bare "Aborted!".
            log_console.print(f"Stopped. {hint} -y takes it all.")
            raise typer.Exit(1)
        if not typer.confirm("Continue anyway?"):
            log_console.print(f"Nothing shared. {hint}")
            raise typer.Exit()

    # [4] Execution
    with spinner(log_console, "[bold blue]Generating Snapshot...[/bold blue]"):

        tree_str = structure.generate_ascii_tree(files, root_path)

        content_cache = cache.ContentCache(root_path)
        file_blocks = []

        for file_path in files:
            try:
                rel_path = file_path.relative_to(root_path).as_posix()
            except ValueError:
                rel_path = f"[EXTERNAL]/{file_path.name}"

            if spans := ranges.get(file_path):
                for lo, hi in spans:
                    content, shown = processor.read_line_range(file_path, lo, hi)
                    file_blocks.append(tags.file(rel_path, content, lines=shown))
                continue

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
            raise typer.Exit(1) from e

    def _summary():
        log_console.print(f"[bold]Snapshot[/bold] {total_files} files, ~{tokenizer.estimate_tokens(snapshot_content):,} tokens")

    handler.handle_output(snapshot_content, summary=_summary, item_name="snapshot")
