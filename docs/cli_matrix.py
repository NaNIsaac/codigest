"""
Write docs/cli-matrix.md and docs/cli-matrix.json: which commands and options each build has.

    uv run python docs/cli_matrix.py            # write both files
    uv run python docs/cli_matrix.py --check    # exit 1 when they are out of date

The Python build is read from Typer the way site/scripts/check_reference.py reads it, the Rust
build from clap through `cargo run --example cli_options` (or a file saved from it, given with
--rust-json). In the JSON, "python" and "rust" map each command path to its option names, the
shape check_reference.py builds from the CLI, so either can stand in for it.
"""

import argparse
import json
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

import click
import typer

from codigest.main import app

DOCS = Path(__file__).resolve().parent
ROOT = DOCS.parent
MARKDOWN = DOCS / "cli-matrix.md"
JSON_FILE = DOCS / "cli-matrix.json"
IGNORED = {"--help", "-h"}

# Options that are in open pull requests or only planned, so neither build on main has them.
PENDING = [
    ("scan", "--github", "#33, not merged", "waits for the decision in DotFiles#81"),
    ("scan", "--ref", "#33, not merged", "waits for the decision in DotFiles#81"),
    ("scan", "--path", "#33, not merged", "waits for the decision in DotFiles#81"),
    ("scan", "--auth", "#33, not merged", "waits for the decision in DotFiles#81"),
    ("scan", "--max-file-size", "#33, not merged", "waits for the decision in DotFiles#81"),
    ("mcp", "--allow-remote", "planned (docs/remote-scan.md in #30)", "planned (same)"),
]

# Not options, so not read from the CLIs: --preset is expanded before either parser sees the
# arguments (src/codigest/core/presets.py, rust/src/presets.rs).
GLOBAL = [("every command", "--preset NAME", "yes", "yes", "both", "expanded before parsing")]

# Behavior that differs without a different option, from rust/README.md ("Differences from the
# Python build"), kept by hand: update it with that section.
BEHAVIOR = [
    (
        "digest",
        "Python and MATLAB outlines",
        "yes",
        "yes",
        "both",
        "",
    ),
    (
        "digest",
        "Rust, JavaScript, TypeScript and Go outlines",
        "no",
        "yes",
        "rust-only",
        "tree-sitter grammars",
    ),
    (
        "digest",
        "outline of a Python file with a syntax error",
        "nothing",
        "what parses",
        "rust-only",
        "",
    ),
    (
        "semdiff",
        "function and class changes",
        "Python files",
        "Python files",
        "both",
        "Python only in both builds",
    ),
    (
        "pack",
        "order of modules called by a failing test",
        "ast.walk order",
        "order of the calls",
        "differs",
        "",
    ),
    (
        "cap",
        "Ctrl+C",
        "default",
        "forwarded, 5 s before kill",
        "differs",
        "",
    ),
    (
        "init, clean, term clear",
        "status line ending on Windows",
        "\\r\\n",
        "\\n",
        "differs",
        "results and -o files use the platform ending in both",
    ),
    (
        "scan",
        "a GitHub repository (OWNER/REPO --github, URL)",
        "#33, not merged",
        "no",
        "pending",
        "network requests, see docs/remote-scan.md",
    ),
]

STATUS_WORDS = {
    "both": "both builds",
    "python-only": "Python build only",
    "rust-only": "Rust build only",
    "hidden": "both builds, not shown by --help",
    "pending": "in an open pull request or planned; not on main",
    "differs": "both builds, with a different result",
}


@dataclass(slots=True, frozen=True)
class Option:
    key: str  # the first long name, or the short name of an option that has none
    names: tuple[str, ...]
    hidden: bool


def python_build() -> dict[str, dict]:
    """Command path -> {"hidden": bool, "options": [Option]}, hidden commands included."""
    found: dict[str, dict] = {}

    def walk(command: click.Command, path: list[str], hidden: bool) -> None:
        options = []
        for param in command.params:
            if not isinstance(param, click.Option):
                continue
            # A --flag/--no-flag pair is two options, as the Rust build defines it.
            for names in (param.opts, param.secondary_opts):
                names = [n for n in names if n not in IGNORED]
                if names:
                    options.append(_option(names, param.hidden))
        found[" ".join(path)] = {"hidden": hidden, "options": options}
        if isinstance(command, click.Group):
            for name, sub in command.commands.items():
                walk(sub, [*path, name], hidden or sub.hidden)

    walk(typer.main.get_command(app), [], False)
    return found


def rust_build(saved: Path | None) -> dict[str, dict]:
    if saved:
        raw = json.loads(saved.read_text(encoding="utf-8"))
    else:
        result = subprocess.run(
            ["cargo", "run", "--quiet", "--example", "cli_options"],
            cwd=ROOT / "rust",
            capture_output=True,
            text=True,
            encoding="utf-8",
            check=True,
        )
        raw = json.loads(result.stdout)
    return {
        path: {
            "hidden": entry["hidden"],
            "options": [_option(o["names"], o["hidden"]) for o in entry["options"]],
        }
        for path, entry in raw.items()
    }


def _option(names: list[str], hidden: bool) -> Option:
    longs = [n for n in names if n.startswith("--")]
    shorts = sorted(n for n in names if not n.startswith("--"))
    key = longs[0] if longs else shorts[0]
    return Option(key, (*longs, *shorts), hidden)


def names_by_path(build: dict[str, dict]) -> dict[str, list[str]]:
    """The shape of check_reference.py's cli_options(): command path -> option names."""
    return {
        path: sorted({n for o in entry["options"] for n in o.names})
        for path, entry in build.items()
        if path
    }


def command_rows(python: dict, rust: dict) -> list[dict]:
    rows = []
    for path in _ordered(python, rust):
        if not path:
            continue
        p, r = python.get(path), rust.get(path)
        if p and r:
            status = "hidden" if p["hidden"] and r["hidden"] else "both"
        else:
            status = "python-only" if p else "rust-only"
        note = ""
        if p and r and p["hidden"] != r["hidden"]:
            note = f"hidden in the {'Python' if p['hidden'] else 'Rust'} build only"
        rows.append(
            {"command": path, "python": bool(p), "rust": bool(r), "status": status, "note": note}
        )
    return rows


def option_rows(python: dict, rust: dict) -> list[dict]:
    rows = []
    for path in _ordered(python, rust):
        p = {o.key: o for o in python.get(path, {}).get("options", [])}
        r = {o.key: o for o in rust.get(path, {}).get("options", [])}
        for key in [*p, *(k for k in r if k not in p)]:
            po, ro = p.get(key), r.get(key)
            notes = []
            if po and ro:
                status = "hidden" if po.hidden and ro.hidden else "both"
                if po.names != ro.names:
                    notes.append(
                        f"names differ: Python {', '.join(po.names)}; Rust {', '.join(ro.names)}"
                    )
                if po.hidden != ro.hidden:
                    notes.append(f"hidden in the {'Python' if po.hidden else 'Rust'} build only")
            else:
                status = "python-only" if po else "rust-only"
                if (po or ro).hidden:
                    notes.append("hidden")
            rows.append(
                {
                    "command": path,
                    "option": key,
                    "python": list(po.names) if po else None,
                    "rust": list(ro.names) if ro else None,
                    "status": status,
                    "note": "; ".join(notes),
                }
            )
    present = {(row["command"], row["option"]) for row in rows}
    for command, option, python_note, rust_note in PENDING:
        if (command, option) in present:
            raise SystemExit(f"{command} {option} is on main now: update PENDING in {__file__}")
        rows.append(
            {
                "command": command,
                "option": option,
                "python": None,
                "rust": None,
                "status": "pending",
                "note": f"Python: {python_note}. Rust: {rust_note}.",
            }
        )
    return rows


def _ordered(python: dict, rust: dict) -> list[str]:
    return [*python, *(path for path in rust if path not in python)]


def build(python: dict, rust: dict) -> tuple[str, str]:
    commands = command_rows(python, rust)
    options = option_rows(python, rust)
    data = {
        "python": names_by_path(python),
        "rust": names_by_path(rust),
        "statuses": STATUS_WORDS,
        "commands": commands,
        "options": options,
        "global": [
            dict(zip(("command", "option", "python", "rust", "status", "note"), g, strict=True))
            for g in GLOBAL
        ],
        "behavior": [
            dict(zip(("command", "what", "python", "rust", "status", "note"), b, strict=True))
            for b in BEHAVIOR
        ],
    }
    return _markdown(commands, options), json.dumps(data, indent=2, ensure_ascii=False) + "\n"


def _cell(names: list[str] | None) -> str:
    return ", ".join(f"`{n}`" for n in names) if names else "–"


def _markdown(commands: list[dict], options: list[dict]) -> str:
    counts = {s: sum(1 for o in options if o["status"] == s) for s in STATUS_WORDS}
    lines = [
        "# Commands and options by build",
        "",
        "Generated by `docs/cli_matrix.py` from the Typer app (Python build) and clap (Rust",
        "build, `rust/examples/cli_options.rs`); do not edit by hand. `cli-matrix.json` holds the",
        "same data. Hidden commands and options are included; `--help` and `-h` are left out.",
        "",
        "| Status | Meaning | Options |",
        "| --- | --- | ---: |",
        *(
            f"| {s} | {w} | {counts[s] if s != 'differs' else 'behavior only'} |"
            for s, w in STATUS_WORDS.items()
        ),
        "",
        "Nothing in either build is marked experimental. Rows marked pending are not on `main`.",
        "",
        "## Commands",
        "",
        "| Command | Python | Rust | Status | Note |",
        "| --- | --- | --- | --- | --- |",
        *(
            f"| `cdg {c['command']}` | {'yes' if c['python'] else 'no'} | "
            f"{'yes' if c['rust'] else 'no'} | {c['status']} | {c['note']} |"
            for c in commands
        ),
        "",
        "## Options",
        "",
    ]
    for path in dict.fromkeys(o["command"] for o in options):
        lines += [
            f"### `cdg {path}`" if path else "### `cdg` (no command)",
            "",
            "| Option | Python | Rust | Status | Note |",
            "| --- | --- | --- | --- | --- |",
            *(
                f"| `{o['option']}` | {_cell(o['python'])} | {_cell(o['rust'])} | "
                f"{o['status']} | {o['note']} |"
                for o in options
                if o["command"] == path
            ),
            "",
        ]
    lines += [
        "## Options outside the parsers",
        "",
        "| Command | Option | Python | Rust | Status | Note |",
        "| --- | --- | --- | --- | --- | --- |",
        *(f"| {c} | `{o}` | {p} | {r} | {s} | {n} |" for c, o, p, r, s, n in GLOBAL),
        "",
        "## Behavior",
        "",
        'Kept by hand from `rust/README.md` ("Differences from the Python build") and open pull',
        "requests.",
        "",
        "| Command | What | Python | Rust | Status | Note |",
        "| --- | --- | --- | --- | --- | --- |",
        *(f"| `{c}` | {w} | {p} | {r} | {s} | {n} |" for c, w, p, r, s, n in BEHAVIOR),
        "",
    ]
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0].strip())
    parser.add_argument("--check", action="store_true", help="fail if the files are out of date")
    parser.add_argument("--rust-json", type=Path, help="output of the cli_options example")
    args = parser.parse_args()

    markdown, data = build(python_build(), rust_build(args.rust_json))
    if args.check:
        stale = [
            f.name
            for f, text in ((MARKDOWN, markdown), (JSON_FILE, data))
            if not f.exists() or f.read_text(encoding="utf-8") != text
        ]
        for name in stale:
            print(f"{name} is out of date: run uv run python docs/cli_matrix.py", file=sys.stderr)
        return 1 if stale else 0
    MARKDOWN.write_text(markdown, encoding="utf-8", newline="\n")
    JSON_FILE.write_text(data, encoding="utf-8", newline="\n")
    print(f"wrote {MARKDOWN.name} and {JSON_FILE.name}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
