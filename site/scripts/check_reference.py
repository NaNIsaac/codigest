"""
Fail when the command reference and the CLI disagree.

Each command has a heading such as ## `cdg scan` or ### `cdg term install`, and its
options are listed in a table whose first cell holds the option names in backticks.
An option that only the prebuilt binary (the Rust build) has gets a heading of its own
under the command instead of a table row, such as #### `--raw`, only in the binary.

The check compares the tables with what Typer registers, in both directions, for the
English page and its Korean translation. With --rust-json it also compares the tables
plus the binary-only headings with the Rust build, read from the "rust" key of
docs/cli-matrix.json.

    uv run python site/scripts/check_reference.py [--rust-json docs/cli-matrix.json]
"""

import argparse
import json
import re
import sys
from pathlib import Path

import click
import typer

from codigest.main import app

SITE = Path(__file__).resolve().parent.parent
PAGES = [SITE / "commands.md", SITE / "ko" / "commands.md"]

HEADING = re.compile(r"^#{2,4}\s+`cdg ([a-z][a-z -]*)`\s*$")
BINARY_ONLY = re.compile(r"^####\s")
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


def documented_options(
    page: Path,
) -> tuple[dict[str, set[str]], dict[str, set[str]], dict[str, set[str]]]:
    """Options in each command's table, and options under its binary-only headings."""
    table: dict[str, set[str]] = {}
    binary: dict[str, set[str]] = {}
    python_only: dict[str, set[str]] = {}
    current: str | None = None
    for line in page.read_text(encoding="utf-8").splitlines():
        if m := HEADING.match(line):
            current = m[1].strip()
            table.setdefault(current, set())
            binary.setdefault(current, set())
            python_only.setdefault(current, set())
        elif current and line.startswith("#### Python-only:"):
            for span in re.findall(r"`([^`]+)`", line):
                python_only[current].update(OPTION.findall(span))
        elif current and BINARY_ONLY.match(line):
            for span in re.findall(r"`([^`]+)`", line):
                binary[current].update(OPTION.findall(span))
        elif current and line.startswith("|"):
            first_cell = line.split("|")[1]
            for span in re.findall(r"`([^`]+)`", first_cell):
                table[current].update(OPTION.findall(span))
    return table, binary, python_only


def compare(
    label: str, build: str, docs: dict[str, set[str]], actual: dict[str, set[str]]
) -> list[str]:
    problems = []
    for command in sorted(actual.keys() - docs.keys()):
        problems.append(f"{label}: command `cdg {command}` is not documented")
    for command in sorted(docs.keys() - actual.keys()):
        problems.append(
            f"{label}: `cdg {command}` is documented but the {build} has no such command"
        )
    for command in sorted(actual.keys() & docs.keys()):
        for option in sorted(actual[command] - docs[command]):
            problems.append(
                f"{label}: `cdg {command}` option {option} of the {build} is not documented"
            )
        for option in sorted(docs[command] - actual[command]):
            problems.append(
                f"{label}: `cdg {command}` documents {option}, which the {build} does not have"
            )
    return problems


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Fail when the command reference and the CLI disagree."
    )
    parser.add_argument(
        "--rust-json", type=Path, help="docs/cli-matrix.json, to check the Rust build too"
    )
    args = parser.parse_args()

    python = cli_options()
    rust = (
        {
            k: set(v) - IGNORED
            for k, v in json.loads(args.rust_json.read_text(encoding="utf-8"))["rust"].items()
        }
        if args.rust_json
        else None
    )
    problems: list[str] = []
    for page in PAGES:
        table, binary, python_only = documented_options(page)
        label = page.relative_to(SITE).as_posix()
        problems += compare(
            label, "Python build", {k: v | python_only[k] for k, v in table.items()}, python
        )
        for command, options in binary.items():
            for option in sorted(options & python.get(command, set())):
                problems.append(
                    f"{label}: `cdg {command}` {option} is marked binary-only but the Python build has it"
                )
        if rust is not None:
            both = {command: table[command] | binary[command] for command in table}
            problems += compare(label, "Rust build", both, rust)
    for problem in problems:
        print(problem, file=sys.stderr)
    if not problems:
        counted = f"{len(python)} commands, {sum(map(len, python.values()))} options"
        if rust is not None:
            counted += f"; Rust build: {sum(map(len, rust.values()))} options"
        print(f"reference matches the CLI: {counted}")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
