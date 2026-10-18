"""
`cdg pack` -- one block with what an LLM needs for the task at hand: the last
failed command run in this project, the files its output points at, your
uncommitted changes, and the local modules those files import. Files are added
in that order until the token budget runs out; whatever is left out is listed,
so the reader knows what it is not seeing.
"""
import re
from datetime import datetime, timedelta, timezone
from pathlib import Path

import typer
from rich.markup import escape

from ..options import CopyOption, MessageOption, StdoutOption, OutputOption

lazy import html
lazy from ..core import common, git_ops, processor, resolver, shadow, tags, termstore, tokenizer
lazy from ..core.redact import redact_diff
lazy from ..core.output import OutputHandler

DEFAULT_BUDGET = 30_000
# A file that is only named in the failing output gets this many lines around each reference.
EXCERPT_CONTEXT = 10
EXCERPT_MIN_LINES = 60
FAILURE_MAX_AGE = timedelta(hours=1)

# `File "x.py", line 7` (Python) or `x.py:7` / `x.ts(7,3)` (pytest, tsc, gcc, rustc, eslint, ...)
_LOCATION_RE = re.compile(r'File "(?P<py>[^"]+)", line (?P<py_line>\d+)|(?P<path>[\w./\\~-]+\.\w+)[:(](?P<line>\d+)')

_TEST_FILE_RE = re.compile(r"^test_.*\.py$|^.*_test\.py$|\.(test|spec)\.[cm]?[jt]sx?$")


SOURCES = ("failure", "changes", "tests", "imports")


def parse_sources(text: str) -> set[str]:
    chosen = {s.strip() for s in text.split(",") if s.strip()}
    if unknown := chosen - set(SOURCES) - {"none"}:
        raise typer.BadParameter(f"unknown source {', '.join(sorted(unknown))}; choose from {', '.join(SOURCES)}")
    return chosen - {"none"}


def pick_failure(entries: list[termstore.Entry], root: Path) -> termstore.Entry | None:
    """
    Latest failed command run under `root` within the last hour, unless the same command has
    passed since. Older failures are rarely what someone running `pack` is looking at.
    """
    cutoff = datetime.now().astimezone() - FAILURE_MAX_AGE
    passed: set[str] = set()
    for entry in reversed(termstore.select(entries, under=root)):
        if _started(entry) < cutoff:
            return None
        if entry.exit_code == 0:
            passed.add(entry.command)
        elif entry.exit_code is not None and entry.command not in passed:
            return entry
    return None


def _started(entry: termstore.Entry) -> datetime:
    try:
        return datetime.fromisoformat(entry.started_at)
    except ValueError:
        return datetime.min.replace(tzinfo=timezone.utc)


def referenced_files(output: str, bases: list[Path], known: set[Path]) -> dict[Path, list[int]]:
    """Project files named in command output, with the line numbers given for each, in order."""
    found: dict[Path, list[int]] = {}
    for m in _LOCATION_RE.finditer(output):
        raw = Path(m["py"] or m["path"]).expanduser()
        for base in bases:
            if (path := (base / raw).resolve()) in known:
                if (line := int(m["py_line"] or m["line"])) not in (lines := found.setdefault(path, [])):
                    lines.append(line)
                break
    return found


def excerpt(path: Path, lines: list[int]) -> tuple[str, str] | None:
    """
    The lines around each reference, numbered, with `...` between gaps, plus the ranges shown
    ("3-23,40-60"). None when the file is short or the excerpt would be most of it anyway.
    """
    try:
        rows = path.read_text(encoding="utf-8").splitlines()
    except (OSError, UnicodeDecodeError):
        return None
    spans: list[list[int]] = []
    for line in sorted(lines):
        lo, hi = max(1, line - EXCERPT_CONTEXT), min(len(rows), line + EXCERPT_CONTEXT)
        if spans and lo <= spans[-1][1] + 1:
            spans[-1][1] = max(spans[-1][1], hi)
        else:
            spans.append([lo, hi])
    if len(rows) <= EXCERPT_MIN_LINES or sum(hi - lo + 1 for lo, hi in spans) > len(rows) * 0.7:
        return None
    width = len(str(len(rows)))
    text = "\n...\n".join(
        "\n".join(f"{i:>{width}}: {rows[i - 1]}" for i in range(lo, hi + 1)) for lo, hi in spans
    )
    return text, ",".join(f"{lo}-{hi}" for lo, hi in spans)


def changed_paths(diff: str, root: Path) -> list[Path]:
    paths = []
    for line in diff.splitlines():
        if line.startswith("diff --git"):
            p = line.split()[-1].removeprefix("b/")
            if (path := (root / p).resolve()).is_file() and path not in paths:
                paths.append(path)
    return paths


def ago(iso: str) -> str:
    try:
        seconds = int((datetime.now().astimezone() - datetime.fromisoformat(iso)).total_seconds())
    except ValueError:
        return ""
    for unit, size in (("d", 86400), ("h", 3600), ("m", 60)):
        if seconds >= size:
            return f"{seconds // size}{unit} ago"
    return "just now"


def working_changes(root: Path, files: list[Path]) -> tuple[str, str, list[Path]]:
    """(diff, where it came from, changed files). A `cdg mark` baseline wins over Git."""
    anchor = shadow.ContextAnchor(root)
    if anchor.has_history():
        diff = anchor.get_changes(files)
        return redact_diff(diff), f"since cdg mark ({anchor.get_last_update_time()})", changed_paths(diff, root)
    if git := git_ops.working_changes(root):
        diff, untracked = git
        return redact_diff(diff), "uncommitted changes (git)", changed_paths(diff, root) + untracked
    return "", "", []


def handle(
    targets: list[Path] = typer.Argument(
        None, help="Files or directories to include first", exists=True, resolve_path=True
    ),
    message: MessageOption = "",
    budget: int = typer.Option(DEFAULT_BUDGET, "-b", "--budget", min=1000, help="Token budget (estimated)"),
    with_: str | None = typer.Option(
        None, "--with", metavar="SOURCES",
        help="Automatic sources to add: failure,changes,tests,imports (all, or none when you name files)",
    ),
    copy: CopyOption = True,
    stdout: StdoutOption = False,
    output: OutputOption = None,
) -> None:
    """
    Bundle the last failure, the files it points at and your changes.

    Examples:

    \b
        cdg pack                             # last failure here + your changes
        cdg pack src/prices.py               # just this file
        cdg pack src/prices.py --with tests  # this file and its tests
        cdg pack --with changes              # only your changes, no failure
        cdg pack -m "fix the discount bug"   # add an instruction
    """
    handler = OutputHandler(stdout=stdout, copy=copy, output=output)
    log = handler.log_console

    ctx = common.get_context(targets)
    root = ctx.root_path
    known = set(ctx.get_target_files())

    # (path, why it is here), most important first
    wanted: dict[Path, str] = {}
    for path in ctx.get_target_files(targets=targets) if targets else []:
        wanted.setdefault(path, "requested")

    # Naming files means "these files": nothing is added unless --with asks for it.
    auto = parse_sources(with_) if with_ is not None else (set() if targets else set(SOURCES))

    refs: dict[Path, list[int]] = {}
    failure = pick_failure(termstore.load_entries(), root) if "failure" in auto else None
    if failure:
        bases = [Path(failure.cwd), root] if failure.cwd else [root]
        refs = referenced_files(failure.output, bases, known)
        for path, lines in refs.items():
            shown = ", ".join(map(str, lines[:3])) + (", ..." if len(lines) > 3 else "")
            wanted.setdefault(path, f"line{'s' * (len(lines) > 1)} {shown} in failing output")

    diff, diff_source, changed_all = working_changes(root, sorted(known)) if "changes" in auto else ("", "", [])
    changed = [p.resolve() for p in changed_all if p.resolve() in known]
    for path in changed:
        wanted.setdefault(path, "changed")

    deps = resolver.DependencyResolver(root)

    def imports(path: Path) -> set[Path]:
        return deps.imports_of(path) & known

    edited = {p for p, why in wanted.items() if why in ("requested", "changed")}
    for test in sorted(p for p in known if _TEST_FILE_RE.search(p.name)) if edited and "tests" in auto else []:
        if hit := sorted(imports(test) & edited):
            wanted.setdefault(test, f"tests {hit[0].relative_to(root).as_posix()}")

    for path in [p for p, why in wanted.items() if not why.endswith("failing output")] if "imports" in auto else []:
        for dep in sorted(imports(path)):
            wanted.setdefault(dep, f"imported by {path.relative_to(root).as_posix()}")

    if not (failure or diff.strip() or wanted):
        log.print("[yellow]Nothing to pack:[/yellow] no failing command here and no changes.")
        log.print("  Capture one with [cyan]cdg cap -- pytest[/cyan], or name files: [cyan]cdg pack src/app.py[/cyan]")
        raise typer.Exit(1)

    head: list[str] = []
    if message:
        head.append(f"<user_instruction>\n{tags.escape_xml_value(message)}\n</user_instruction>")
    if failure:
        head.append(termstore.render([failure]))
    if diff.strip():
        head.append(f'<changes source="{html.escape(diff_source)}">\n{tags.escape_xml_value(diff.rstrip())}\n</changes>')
    used = tokenizer.estimate_tokens("\n".join(head))

    blocks: list[str] = []
    included: list[tuple[str, str]] = []
    omitted: list[tuple[str, str]] = []
    # Credential files never get here (file selection withholds them); name the ones that
    # were asked for or changed, so it is clear they were left out on purpose.
    named_or_changed = {p.resolve() for p in targets or []} | {p.resolve() for p in changed_all}
    for path in ctx.withheld:
        if path.resolve() in named_or_changed or any(path.is_relative_to(t.resolve()) for t in targets or [] if t.is_dir()):
            omitted.append((path.relative_to(root).as_posix(), "may contain secrets"))
    for path, reason in wanted.items():
        rel = path.relative_to(root).as_posix() if path.is_relative_to(root) else str(path)
        # Files you changed or asked for come whole; files the output only points at come as excerpts.
        part = excerpt(path, refs[path]) if path in refs and reason.endswith("failing output") and path not in changed else None
        content, shown = part or (processor.read_file_content(path, add_line_numbers=True), "")
        lines_attr = f' lines="{shown}"' if shown else ""
        block = f'<file path="{html.escape(rel)}" reason="{html.escape(reason)}"{lines_attr}>\n{tags.escape_xml_value(content)}\n</file>'
        cost = tokenizer.estimate_tokens(block)
        if used + cost > budget:
            omitted.append((rel, f"over budget (~{cost:,} tokens)"))
            continue
        used += cost
        blocks.append(block)
        brief = f"{len(refs[path])} places in failing output" if reason.startswith("lines ") else reason
        included.append((rel, f"{brief} (excerpt)" if shown else brief))

    if omitted:
        listing = "\n".join(f"{rel}: {why}" for rel, why in omitted)
        blocks.append(f"<omitted>\n{tags.escape_xml_value(listing)}\n</omitted>")

    pack = f'<context_pack project="{html.escape(root.name)}">\n' + "\n".join(head + blocks) + "\n</context_pack>"

    over = " [red](fixed parts alone exceed the budget)[/red]" if used > budget else ""
    log.print(f"[bold]Packed ~{used:,} / {budget:,} tokens[/bold]{over}")
    if auto != set(SOURCES):
        on = ", ".join(s for s in SOURCES if s in auto) or "none"
        log.print(f"  [dim]auto     {on} (--with {','.join(SOURCES)} adds more)[/dim]")
    if failure:
        when = ago(failure.started_at)
        log.print(f"  [red]failed [/red]  [cyan]{escape(failure.command)}[/cyan] [dim](exit {failure.exit_code}{', ' + when if when else ''})[/dim]")
    if diff.strip():
        log.print(f"  [yellow]changes[/yellow]  [dim]{diff_source}[/dim]")
    width = max((len(rel) for rel, _ in included + omitted), default=0)
    for rel, why in included:
        log.print(f"  [green]+[/green] {escape(rel):<{width}}  [dim]{escape(why)}[/dim]")
    for rel, why in omitted:
        log.print(f"  [dim]- {escape(rel):<{width}}  {escape(why)}[/dim]")

    handler.handle_output(pack, item_name="pack")
