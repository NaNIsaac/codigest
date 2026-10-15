"""
Centralized CLI Options & Common Annotated Types.
"""
from typing import Annotated
import typer

# Plain aliases, not PEP 695 `type X = ...`: Typer can't unwrap TypeAliasType.
CopyOption = Annotated[
    bool,
    typer.Option("--copy/--no-copy", "-c", help="Auto-copy output to clipboard")
]

StdoutOption = Annotated[
    bool,
    typer.Option("-s", "--stdout", help="Print output to terminal (stdout) instead of file")
]

ResolveOption = Annotated[
    bool,
    typer.Option("-r", "--resolve", help="Recursively resolve imports")
]

MessageOption = Annotated[
    str,
    typer.Option("-m", "--message", help="Add specific instruction context")
]

AllOption = Annotated[
    bool,
    typer.Option("-a", "--all", help="Ignore config filters")
]
