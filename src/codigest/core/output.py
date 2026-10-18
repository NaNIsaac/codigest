"""
Unified Output Handler for CLI commands.
Manages stderr log console, clipboard copying, and file vs stdout output.
"""
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path
lazy import pyperclip
from rich.console import Console

from .redact import redact_code

console = Console()
err_console = Console(stderr=True)

@dataclass(slots=True)
class OutputHandler:
    stdout: bool = False
    copy: bool = True
    output: Path | None = None

    @property
    def log_console(self) -> Console:
        """
        Returns err_console (stderr) when output is directed to stdout,
        ensuring progress/logs do not pollute stdout pipelines.
        """
        return err_console if self.stdout else console

    def copy_to_clipboard(self, content: str, item_name: str = "output") -> None:
        """Copies content to clipboard safely."""
        if not self.copy:
            return
        try:
            pyperclip.copy(content)
            self.log_console.print(f"[dim]Copied {item_name} to clipboard[/dim]")
        except Exception:
            self.log_console.print("[yellow]Clipboard unavailable; use -s to print instead[/yellow]")

    def handle_output(
        self, 
        content: str, 
        summary: Callable[[], None] | None = None,
        item_name: str = "output"
    ) -> None:
        """
        Clipboard unless --no-copy, the file given with -o, and stdout with -s.
        `summary` prints a short report instead of the content when not on stdout.
        """
        # Every command's context passes through here, so this is the one place that
        # guarantees no unambiguous token (API key, private key, URL password) leaves.
        content = redact_code(content)
        self.copy_to_clipboard(content, item_name=item_name)
        if self.output:
            self.output.write_text(content + "\n", encoding="utf-8")
            self.log_console.print(f"[dim]Wrote {item_name} to {self.output}[/dim]", highlight=False)

        if self.stdout:
            print(content)
        elif summary:
            summary()
