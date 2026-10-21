"""
Fail when the command reference and the CLI disagree.

Each command has a heading such as ## `cdg scan` or ### `cdg term install`, and its
options are listed in a table whose first cell holds the option names in backticks.
The check compares those names with what Typer registers, in both directions, for the
English page and its Korean translation.

    uv run python site/scripts/check_reference.py
"""

import re
import sys
from pathlib import Path

import click
import typer

from codigest.main import app

SITE = Path(__file__).resolve().parent.parent
PAGES = [SITE / "commands.md", SITE / "ko" / "commands.md"]

HEADING = re.compile(r"^#{2,4}\s+`cdg ([a-z][a-z -]*)`\s*$")
OPTION = re.compile(r"(?<![\w-])(--?[A-Za-z][\w-]*)")
IGNORED = {"--help"}


def cli_options() -> dict[str, set[str]]:
    """Every command path (hidden ones included) mapped to its option names."""
    found: dict[str, set[str]] = {}

    def walk(command: click.Command, path: list[str]) -> None:
        if isinstance(command, click.Group):
            for name, sub in command.commands.items():
                walk(sub, [*path, name])
        if path:
            names = {
                n
                for p in command.params
                if isinstance(p, click.Option)
                for n in (*p.opts, *p.secondary_opts)
            }
            found[" ".join(path)] = names - IGNORED

    walk(typer.main.get_command(app), [])
    return found


def documented_options(page: Path) -> dict[str, set[str]]:
    found: dict[str, set[str]] = {}
    current: str | None = None
    for line in page.read_text(encoding="utf-8").splitlines():
        if m := HEADING.match(line):
            current = m[1].strip()
            found.setdefault(current, set())
        elif current and line.startswith("|"):
            first_cell = line.split("|")[1]
            for span in re.findall(r"`([^`]+)`", first_cell):
                found[current].update(OPTION.findall(span))
    return found


def main() -> int:
    actual = cli_options()
    problems: list[str] = []
    for page in PAGES:
        docs = documented_options(page)
        label = page.relative_to(SITE).as_posix()
        for command in sorted(actual.keys() - docs.keys()):
            problems.append(f"{label}: command `cdg {command}` is not documented")
        for command in sorted(docs.keys() - actual.keys()):
            problems.append(
                f"{label}: `cdg {command}` is documented but the CLI has no such command"
            )
        for command in sorted(actual.keys() & docs.keys()):
            for option in sorted(actual[command] - docs[command]):
                problems.append(f"{label}: `cdg {command}` option {option} is not documented")
            for option in sorted(docs[command] - actual[command]):
                problems.append(
                    f"{label}: `cdg {command}` documents {option}, which the CLI does not have"
                )
    for problem in problems:
        print(problem, file=sys.stderr)
    if not problems:
        print(
            f"reference matches the CLI: {len(actual)} commands, {sum(map(len, actual.values()))} options"
        )
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
