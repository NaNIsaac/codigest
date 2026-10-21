"""
Unified Output Handler for CLI commands.
Manages stderr log console, clipboard copying, and file vs stdout output.
"""
import base64
import contextlib
import os
import sys
from collections.abc import Callable, Iterator
from dataclasses import dataclass
from pathlib import Path
lazy import pyperclip
from rich.console import Console

from .redact import redact_code

console = Console()
err_console = Console(stderr=True)


@contextlib.contextmanager
def spinner(log: Console, text: str) -> Iterator[None]:
    """A transient spinner in a terminal and nothing elsewhere: rich's Progress leaves a
    blank line behind when its output is a pipe or a log, even when disabled."""
    if not log.is_terminal:
        yield
        return
    from rich.progress import Progress, SpinnerColumn, TextColumn

    with Progress(SpinnerColumn(), TextColumn(text), transient=True, console=log) as progress:
        progress.add_task(text, total=None)
        yield


def use_osc52() -> bool:
    """Over SSH the system clipboard is the remote machine's, not the one of the person typing.
    `CODIGEST_CLIPBOARD=osc52` or `=system` overrides the guess."""
    match os.environ.get("CODIGEST_CLIPBOARD"):
        case "osc52":
            return True
        case "system":
            return False
    return any(name in os.environ for name in ("SSH_CONNECTION", "SSH_CLIENT", "SSH_TTY"))


def osc52(text: str) -> str:
    """The sequence that asks the terminal to put `text` on its own clipboard."""
    return f"\x1b]52;c;{base64.b64encode(text.encode()).decode()}\x07"


def send_osc52(text: str) -> None:
    # Written to the terminal itself, so it arrives even when stdout is a pipe.
    with open("CONOUT$" if os.name == "nt" else "/dev/tty", "w", encoding="ascii") as tty:
        tty.write(osc52(text))


@dataclass(slots=True)
class OutputHandler:
    stdout: bool = False
    copy: bool | None = True
    output: Path | None = None

    def __post_init__(self) -> None:
        # Redirected or run by an agent's shell: whoever reads the pipe needs the text, and
        # silently replacing the user's clipboard from a background process would be a surprise.
        # `copy=None` means "not chosen on the command line".
        if not sys.stdout.isatty():
            self.stdout = True
            if self.copy is None:
                self.copy = False
        elif self.copy is None:
            self.copy = True

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
        if use_osc52():
            try:
                send_osc52(content)
                self.log_console.print(f"[dim]Sent {item_name} to the terminal clipboard (OSC 52)[/dim]")
            except OSError:
                self.log_console.print(
                    "[yellow]No terminal to send the clipboard to; use -s to print instead[/yellow]"
                )
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
