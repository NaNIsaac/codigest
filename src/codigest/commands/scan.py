import re
import sys
from pathlib import Path

import typer

from ..core.formats import Format, fenced
from ..options import (
    CopyOption,
    FormatOption,
    StdoutOption,
    ResolveOption,
    MessageOption,
    AllOption,
    OutputOption,
)

lazy from rich.filesize import decimal
lazy import json
lazy from ..core import structure, tags, prompts, processor, tokenizer, common, cache, remote
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
            if re.fullmatch(r"[A-Za-z0-9-]+/[A-Za-z0-9._-]+", item):
                hint = f"; add --github to scan the GitHub repository {item}"
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
    line_numbers: bool = typer.Option(
        False, "--lines", "-l", help="Add line numbers to code blocks"
    ),
    yes: bool = typer.Option(False, "-y", "--yes", help="Skip confirmation prompt"),
    resolve: ResolveOption = False,
    format: FormatOption = Format.XML,
    github: bool = typer.Option(
        False,
        "--github",
        help="Read the target OWNER/REPO from GitHub (implied by a github.com URL)",
    ),
    ref: str | None = typer.Option(
        None,
        "--ref",
        help="GitHub: branch, tag or commit [default: the repository's default branch]",
        show_default=False,
    ),
    paths: list[str] = typer.Option(
        None,
        "--path",
        help="GitHub: only this path of the repository; repeat for more, FILE:40-60 for lines",
        show_default=False,
    ),
    auth: bool = typer.Option(
        False,
        "--auth",
        help="GitHub: send your token (GH_TOKEN, GITHUB_TOKEN or gh) to read private repositories",
    ),
    max_file_size: str = typer.Option(
        "1MB", "--max-file-size", help="GitHub: list larger files as skipped, without downloading"
    ),
) -> None:
    """
    Bundle the whole project, or only the given paths, as one block.
    A GitHub repository (OWNER/REPO --github, or its URL) is read the same way, without cloning.
    """
    handler = OutputHandler(stdout=stdout, copy=copy, output=output)
    log_console = handler.log_console
    raw_targets = raw_targets or []

    if target := _remote_target(raw_targets, github, ref, paths, resolve):
        try:
            limit = remote.parse_size(max_file_size)
        except ValueError as e:
            raise typer.BadParameter(str(e), param_hint="'--max-file-size'") from None
        try:
            _scan_remote(handler, target, auth, limit, yes, message, line_numbers, format)
        except remote.RemoteError as e:
            log_console.print(f"[red]Error:[/red] {e}", highlight=False)
            raise typer.Exit(1) from None
        return

    targets, ranges = _parse_targets(raw_targets)

    # [1] Context Setup (Centralized)
    ctx = common.get_context(targets)
    root_path = ctx.root_path

    # [2] File Discovery via Context
    with spinner(log_console, "[bold blue]Scanning...[/bold blue]"):
        files = ctx.get_target_files(targets=targets, ignore_config=all, resolve_deps=resolve)

    # [3] Pre-flight Check
    # A range of a big file is small: count what will be shared, not the whole file.
    total_size = sum(
        sum(len(processor.read_line_range(f, lo, hi)[0]) for lo, hi in ranges[f])
        if f in ranges
        else f.stat().st_size
        for f in files
    )

    if ctx.withheld:
        _report(log_console, "Withheld (may contain secrets)", ctx.withheld, root_path)
    if not yes:
        _confirm_size(log_console, len(files), total_size)

    _render(handler, root_path, files, ranges, message, line_numbers, format)


def _report(log_console, label: str, paths, root: Path | None = None) -> None:
    names = ", ".join(p.relative_to(root).as_posix() if root else p for p in paths)
    log_console.print(f"[yellow]{label}:[/yellow] {names}", highlight=False)


def _confirm_size(log_console, total_files: int, total_size: int) -> None:
    """Asks before sharing a large context; stops when nobody can answer."""
    TOKEN_THRESHOLD = 30000
    FILE_COUNT_THRESHOLD = 100
    est_tokens = int(total_size / 4)
    if est_tokens <= TOKEN_THRESHOLD and total_files <= FILE_COUNT_THRESHOLD:
        return
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


def _remote_target(
    raw: list[str], github: bool, ref: str | None, paths: list[str] | None, resolve: bool
) -> remote.Target | None:
    """The repository to read from GitHub, or None for an ordinary local scan."""
    is_remote = github or any(remote.is_url(r) for r in raw)
    if not is_remote:
        if ref is not None or paths:
            raise typer.BadParameter(
                "--ref and --path need a GitHub repository (OWNER/REPO --github, or its URL)",
                param_hint="targets",
            )
        return None
    if len(raw) != 1:
        raise typer.BadParameter(
            "name one GitHub repository and nothing else; choose its paths with --path",
            param_hint="targets",
        )
    if resolve:
        raise typer.BadParameter("-r does not work on a GitHub repository yet", param_hint="'-r'")
    try:
        target = remote.parse_target(raw[0])
        if ref is not None:
            target.ref = ref
        if paths:
            target.paths = [remote.repo_target(p) for p in paths]
    except ValueError as e:
        raise typer.BadParameter(str(e), param_hint="'--path'" if paths else "targets") from None
    return target


def _scan_remote(
    handler,
    target: remote.Target,
    auth: bool,
    max_file_size: int,
    yes: bool,
    message: str,
    line_numbers: bool,
    format: Format,
) -> None:
    log_console = handler.log_console
    with spinner(log_console, f"[bold blue]Listing {target.name}...[/bold blue]"):
        plan = remote.plan(target, auth=auth, max_file_size=max_file_size)

    for note in plan.notes:
        log_console.print(f"[yellow]Note:[/yellow] {note}", highlight=False)
    if plan.withheld:
        _report(log_console, "Withheld (may contain secrets)", plan.withheld)
    for reason, skipped in plan.skipped.items():
        _report(log_console, f"Skipped ({reason})", skipped)
    if len(plan.files) > remote.MAX_FILES or plan.size > remote.MAX_BYTES:
        raise remote.RemoteError(
            f"{len(plan.files)} files, {decimal(plan.size)} selected; a GitHub scan takes at most "
            f"{remote.MAX_FILES:,} files or {decimal(remote.MAX_BYTES)}. Name folders with --path."
        )
    if not yes:
        _confirm_size(log_console, len(plan.files), plan.size)

    with (
        spinner(log_console, f"[bold blue]Downloading {plan.name}...[/bold blue]"),
        remote.materialize(plan) as root,
    ):
        # Paths named with --path become targets inside the folder, line ranges included. A
        # named path with nothing selected under it (ignored, withheld) was not downloaded; the
        # local scan of such a path shares nothing, so it is dropped, not scanned as the root.
        named = [
            str(root.joinpath(*path.split("/"))) + span
            for path, span in plan.targets
            if root.joinpath(*path.split("/")).exists()
        ]
        targets, ranges = _parse_targets(named)
        ctx = common.get_context(targets, root=root)
        files = [] if plan.targets and not named else ctx.get_target_files(targets=targets)
        _render(
            handler,
            root,
            files,
            ranges,
            message,
            line_numbers,
            format,
            project=plan.name,
            source={"source": plan.source, "ref": plan.ref, "commit": plan.commit},
            label=f"{plan.name}@{plan.ref} ({plan.commit[:7]}) ",
        )


def _render(
    handler,
    root_path: Path,
    files: list[Path],
    ranges,
    message: str,
    line_numbers: bool,
    format: Format,
    project: str | None = None,
    source: dict[str, str] | None = None,
    label: str = "",
) -> None:
    """The snapshot of the selected files, handed to the output. `source` (url, ref and commit)
    is set only for a repository read from GitHub."""
    log_console = handler.log_console
    project = project or root_path.name
    source = source or {}
    total_files = len(files)

    if format != Format.XML:
        with spinner(log_console, "[bold blue]Generating Snapshot...[/bold blue]"):
            tree_str = structure.generate_ascii_tree(files, root_path)
            # The block cache holds XML blocks, so other formats read the files afresh.
            shared = _shared_files(files, ranges, root_path, line_numbers)
            render = _markdown if format == Format.MARKDOWN else _json
            snapshot_content = render(project, message, tree_str, shared, source)
        _deliver(handler, snapshot_content, total_files, label)
        return

    prompt_engine = prompts.get_engine(root_path)

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
                rel_path=rel_path, line_numbers=line_numbers, file_path=file_path
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
                        file_path=file_path,
                    )
                except Exception:
                    continue

        content_cache.save()

        source_code_blob = "\n\n".join(file_blocks)

        try:
            snapshot_content = prompt_engine.render(
                "snapshot",
                project_name=project,
                tree_structure=tree_str,
                source_code=source_code_blob,
                instruction=message,
                **source,
            )
        except Exception as e:
            log_console.print(f"[red][Error] Template Rendering Failed:[/red] {e}")
            raise typer.Exit(1) from e

    _deliver(handler, snapshot_content, total_files, label)


def _deliver(handler, snapshot: str, total_files: int, label: str = "") -> None:
    def summary():
        tokens = tokenizer.estimate_tokens(snapshot)
        handler.log_console.print(
            f"[bold]Snapshot[/bold] {label}{total_files} files, ~{tokens:,} tokens",
            highlight=False,
        )

    handler.handle_output(snapshot, summary=summary, item_name="snapshot")


def _shared_files(files, ranges, root: Path, line_numbers: bool) -> list[dict[str, str]]:
    """One entry per file, or per line range of it, for the Markdown and JSON formats."""
    shared = []
    for file_path in files:
        try:
            rel_path = file_path.relative_to(root).as_posix()
        except ValueError:
            rel_path = f"[EXTERNAL]/{file_path.name}"
        if spans := ranges.get(file_path):
            for lo, hi in spans:
                content, shown = processor.read_line_range(file_path, lo, hi)
                shared.append({"path": rel_path, "lines": shown, "content": content})
        else:
            content = processor.read_file_content(file_path, add_line_numbers=line_numbers)
            shared.append({"path": rel_path, "lines": "", "content": content})
    return shared


def _markdown(
    project: str,
    instruction: str,
    tree: str,
    files: list[dict[str, str]],
    source: dict[str, str] | None = None,
) -> str:
    parts = [f"# {project}"]
    if source:
        parts.append(f"Source: {source['source']}, {source['ref']} at {source['commit']}")
    if instruction:
        parts.append(instruction)
    parts.append(f"## Structure\n\n{fenced(tree)}")
    for file in files:
        heading = f"## {file['path']}"
        if file["lines"]:
            heading += f" (lines {file['lines']})"
        parts.append(f"{heading}\n\n{fenced(file['content'], file['path'])}")
    return "\n\n".join(parts) + "\n"


def _json(
    project: str,
    instruction: str,
    tree: str,
    files: list[dict[str, str]],
    source: dict[str, str] | None = None,
) -> str:
    return json.dumps(
        {
            "project": project,
            **(
                {
                    "source": {
                        "url": source["source"],
                        "ref": source["ref"],
                        "commit": source["commit"],
                    }
                }
                if source
                else {}
            ),
            "instruction": instruction,
            "structure": tree,
            "files": [{k: v for k, v in file.items() if v or k != "lines"} for file in files],
        },
        indent=2,
        ensure_ascii=False,
    )
