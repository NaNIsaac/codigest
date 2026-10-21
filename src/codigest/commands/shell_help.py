"""Optional, reversible PowerShell comment-based help integration."""
from pathlib import Path

import typer
from rich.console import Console

from . import term

app = typer.Typer(help="Let PowerShell Get-Help find codigest without enabling capture.")
console = Console()

_BEGIN = "# >>> codigest PowerShell help >>>"
_END = "# <<< codigest PowerShell help <<<"
_HELP_SCRIPT = Path(__file__).resolve().parent.parent / "shell" / "help.ps1"


def _without_block(text: str) -> str:
    """Remove only the managed help block, preserving everything else."""
    start = text.find(_BEGIN)
    end = text.find(_END, start + len(_BEGIN))
    if start < 0 or end < 0:
        return text
    return (text[:start] + text[end + len(_END):]).strip("\n") + "\n"


def _configure(installing: bool) -> None:
    profiles = term._powershell_profiles()
    if not profiles:
        console.print("[red]PowerShell not found. Install PowerShell first.[/red]")
        raise typer.Exit(code=1)

    path = str(_HELP_SCRIPT).replace("'", "''")
    source = f"if (Test-Path -LiteralPath '{path}') {{ . '{path}' }}"
    for target in dict.fromkeys(profiles):
        try:
            original = target.read_bytes().decode("utf-8-sig") if target.exists() else ""
        except UnicodeDecodeError:
            console.print(f"[yellow]Skipped non-UTF-8 profile:[/yellow] {target}")
            continue
        updated = _without_block(original)
        if installing:
            updated = ((updated.rstrip("\n") + "\n\n") if updated.strip() else "") + (
                f"{_BEGIN}\n{source}\n{_END}\n"
            )
        elif updated == original:
            continue
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(updated.encode("utf-8-sig"))
        console.print(f"{'Installed' if installing else 'Removed'} PowerShell help in {target}")
    console.print("Open a new PowerShell window to apply the change.")


@app.command("install")
def install() -> None:
    """Register PowerShell help for codigest (opt-in)."""
    _configure(installing=True)


@app.command("uninstall")
def uninstall() -> None:
    """Remove only the Codigest PowerShell help integration."""
    _configure(installing=False)
