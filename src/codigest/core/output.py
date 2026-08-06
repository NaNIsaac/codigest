"""
Unified Output Handler for CLI commands.
Manages stderr log console, clipboard copying, and file vs stdout output.
"""
from dataclasses import dataclass
from typing import Callable, Optional
import pyperclip
from rich.console import Console

console = Console()
err_console = Console(stderr=True)

@dataclass
class OutputHandler:
    stdout: bool = False
    copy: bool = True

    @property
    def log_console(self) -> Console:
        """
        Returns err_console (stderr) when output is directed to stdout,
        ensuring progress/logs do not pollute stdout pipelines.
        """
        return err_console if self.stdout else console

    def copy_to_clipboard(self, content: str, item_name: str = "output"):
        """Copies content to clipboard safely."""
        if not self.copy:
            return
        try:
            pyperclip.copy(content)
            self.log_console.print(f"[dim]📋 Copied {item_name} to clipboard[/dim]")
        except Exception:
            self.log_console.print("[dim]⚠️ Clipboard unavailable (skipped copy)[/dim]")

    def handle_output(
        self, 
        content: str, 
        save_action: Optional[Callable[[], None]] = None,
        item_name: str = "output"
    ):
        """
        Executes clipboard copy, stdout print, or file save action cleanly.
        """
        self.copy_to_clipboard(content, item_name=item_name)

        if self.stdout:
            print(content)
        elif save_action:
            save_action()
