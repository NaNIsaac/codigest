"""
Transparent Command Runner.

Executes a command while mirroring its combined stdout/stderr to this process's
stdout in real time, so the user's terminal looks unchanged, and returning the
captured bytes for storage.

stdout and stderr are merged into a single pipe on purpose: the interleaved
stream is what the user actually saw, and that ordering is what makes the
capture useful as LLM context.
"""
import os
lazy import shlex
lazy import shutil
import subprocess
import sys
import time
from pathlib import Path

_SHELL_METACHARS = set("|&><;$`*?()")
_READ_SIZE = 65536


def join_command(args: list[str]) -> str:
    """Renders argv back into the command line the user effectively typed."""
    if needs_shell(args):
        return args[0]
    if sys.platform == "win32":
        return subprocess.list2cmdline(args)
    return shlex.join(args)


def needs_shell(args: list[str]) -> bool:
    """
    A single argument carrying shell metacharacters is a pipeline the user typed
    as one string (e.g. `cdg cap "ls | head"`), so it must go through a shell.
    Multiple args are an already-tokenized argv and are executed directly.
    """
    if len(args) != 1:
        return False
    return any(ch in _SHELL_METACHARS for ch in args[0])


def resolve_shell() -> list[str] | None:
    """
    Returns the argv prefix for running a command string through a shell.
    Honors CODIGEST_SHELL, then prefers the platform's interactive shell.
    """
    if override := os.environ.get("CODIGEST_SHELL"):
        if sys.platform == "win32" and Path(override).stem.lower() in ("pwsh", "powershell"):
            return [override, "-NoProfile", "-Command"]
        return [override, "-c"]

    if sys.platform == "win32":
        # Most Codigest users on Windows type into PowerShell, so pipeline
        # semantics should match PowerShell rather than cmd.exe.
        for candidate in ("pwsh", "powershell"):
            if found := shutil.which(candidate):
                return [found, "-NoProfile", "-Command"]
        comspec = os.environ.get("COMSPEC")
        return [comspec, "/c"] if comspec else None

    shell = os.environ.get("SHELL") or shutil.which("bash") or "/bin/sh"
    return [shell, "-c"]


def build_argv(args: list[str]) -> list[str]:
    """Maps user-supplied arguments onto an argv for subprocess."""
    if not needs_shell(args):
        return args
    if not (prefix := resolve_shell()):
        return args
    return [*prefix, args[0]]


def run(args: list[str]) -> tuple[int, str, int]:
    """
    Runs the command, streaming output through. Returns
    (exit_code, captured_output, duration_ms).

    A KeyboardInterrupt is forwarded to the child and the partial output is
    still returned, since an interrupted run is often exactly what the user
    wants to hand to an LLM.
    """
    argv = build_argv(args)
    chunks: list[bytes] = []
    started = time.monotonic()

    try:
        process = subprocess.Popen(
            argv,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            bufsize=0,
        )
    except FileNotFoundError as e:
        e.add_note(f"argv: {argv}")
        raise
    except OSError as e:
        raise RuntimeError(f"Failed to launch command: {e}") from e

    out_stream = getattr(sys.stdout, "buffer", None)

    try:
        assert process.stdout is not None
        # bufsize=0 yields a raw FileIO, so read one syscall at a time to keep
        # output flowing live instead of blocking for a full buffer.
        fd = process.stdout.fileno()
        while True:
            try:
                if not (chunk := os.read(fd, _READ_SIZE)):
                    break
            except OSError:
                break
            chunks.append(chunk)
            if out_stream is not None:
                out_stream.write(chunk)
                out_stream.flush()
            else:
                sys.stdout.write(chunk.decode("utf-8", errors="replace"))
                sys.stdout.flush()
        exit_code = process.wait()
    except KeyboardInterrupt:
        process.terminate()
        try:
            exit_code = process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            exit_code = process.wait()
    finally:
        if process.stdout is not None:
            process.stdout.close()

    duration_ms = int((time.monotonic() - started) * 1000)
    output = b"".join(chunks).decode("utf-8", errors="replace")
    return exit_code, output, duration_ms
