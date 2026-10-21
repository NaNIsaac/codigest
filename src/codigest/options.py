"""
Centralized CLI Options & Common Annotated Types.
"""

from pathlib import Path
from typing import Annotated
import typer

from .core.formats import Format

# Plain aliases, not PEP 695 `type X = ...`: Typer can't unwrap TypeAliasType.
CopyOption = Annotated[
    bool | None,
    typer.Option(
        "--copy/--no-copy",
        "-c",
        help="Copy the result to the clipboard [default: copy in a terminal, not when piped]",
        show_default=False,
    ),
]

StdoutOption = Annotated[
    bool, typer.Option("-s", "--stdout", help="Also print the result (on by default when piped)")
]

ResolveOption = Annotated[bool, typer.Option("-r", "--resolve", help="Recursively resolve imports")]

MessageOption = Annotated[
    str, typer.Option("-m", "--message", help="Add specific instruction context")
]

AllOption = Annotated[bool, typer.Option("-a", "--all", help="Ignore config filters")]

OutputOption = Annotated[
    Path | None,
    typer.Option("-o", "--output", help="Also write the result to this file", dir_okay=False),
]

FormatOption = Annotated[
    Format,
    typer.Option("--format", help="Output format"),
]
