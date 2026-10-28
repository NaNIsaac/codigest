"""The capture hooks in an interactive shell on a pseudo-terminal: a line they wrap gives the
programs it runs a pipe for stdout, a skipped one leaves them the terminal. hanfix's `hf`
re-runs the previous command, which may be one that needs the terminal, so it is skipped."""

import os
import shutil
import subprocess
import sys
import threading
import time
from pathlib import Path

import pytest

SHELL_DIR = Path(__file__).parents[1] / "src" / "codigest" / "shell"
PROBE = 'import os, sys\nwith open(sys.argv[1], "a") as f: f.write(f"{sys.argv[2]} {os.isatty(1)}\\n")\n'
KOREAN_HF = "ㅗㄹ"  # hf typed with the Korean layout on


class Pty:
    """An interactive program on a pseudo-terminal: stdlib pty on POSIX, ConPTY on Windows."""

    def __init__(self, argv, env):
        self.output = []
        if os.name == "nt":
            winpty = pytest.importorskip("winpty", reason="pywinpty is needed for a ConPTY")
            self.proc = winpty.PtyProcess.spawn(argv, env=env, dimensions=(40, 200))
            self._write = lambda text: self.proc.write(text)
            self._read = lambda: self.proc.read(4096)
        else:
            import pty

            pid, self.fd = pty.fork()
            if pid == 0:
                os.execvpe(argv[0], argv, env)
            self.pid = pid
            self._write = lambda text: os.write(self.fd, text.encode())
            self._read = lambda: os.read(self.fd, 4096).decode(errors="replace")
        threading.Thread(target=self._pump, daemon=True).start()

    def _pump(self):
        while True:
            try:
                chunk = self._read()
            except EOFError, OSError:
                return
            if not chunk:
                return
            self.output.append(chunk)

    def type_lines(self, lines, pause):
        for line in lines:
            time.sleep(pause)
            self._write(line + "\r")

    def wait(self, timeout):
        deadline = time.time() + timeout
        if os.name == "nt":
            while self.proc.isalive() and time.time() < deadline:
                time.sleep(0.2)
            self.proc.terminate(force=True)
        else:
            while time.time() < deadline:
                if os.waitpid(self.pid, os.WNOHANG)[0]:
                    return
                time.sleep(0.2)
            os.kill(self.pid, 9)
            os.waitpid(self.pid, 0)


def fake_cdg(bin_dir: Path):
    # The hooks only load when cdg is on PATH; ingest just reads what it is given.
    script = bin_dir / "cdg"
    script.write_text("#!/bin/sh\ncat >/dev/null\n", encoding="utf-8")
    script.chmod(0o755)
    (bin_dir / "cdg.cmd").write_text("@more >nul\r\n", encoding="utf-8")


def run_shell(tmp_path, argv, lines, pause):
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir()
    fake_cdg(bin_dir)
    env = dict(os.environ)
    env.update(
        PATH=f"{bin_dir}{os.pathsep}{env['PATH']}", HOME=str(tmp_path), TERM="xterm-256color"
    )
    for name in ("CODIGEST_NOCAP", "CODIGEST_CAP_SKIP"):
        env.pop(name, None)
    term = Pty(argv, env)
    term.type_lines(lines, pause)
    term.wait(30)
    return "".join(term.output)[-2000:]


def results(path: Path):
    lines = path.read_text(encoding="utf-8").split() if path.exists() else []
    return dict(zip(lines[::2], lines[1::2], strict=True))


def test_hook_ps1_leaves_hf_the_terminal(tmp_path):
    pwsh = shutil.which("pwsh")
    if not pwsh:
        pytest.skip("pwsh not installed")
    out = tmp_path / "probe.txt"
    probe = tmp_path / "probe.py"
    probe.write_text(PROBE, encoding="utf-8")
    run = f"& '{sys.executable}' '{probe}' '{out}'"
    lines = [
        f"function wrapped {{ {run} wrapped }}",
        f"function hf {{ {run} $args[0] }}",
        f"Set-Alias {KOREAN_HF} hf",
        f". '{SHELL_DIR / 'hook.ps1'}'",
        "wrapped",
        "hf hf",
        f"{KOREAN_HF} alias",
        "exit",
    ]
    screen = run_shell(tmp_path, [pwsh, "-NoProfile", "-NoLogo"], lines, pause=2.5)
    assert results(out) == {"wrapped": "False", "hf": "True", "alias": "True"}, screen


@pytest.mark.parametrize("shell", ["bash", "zsh"])
def test_hook_sh_leaves_hf_the_terminal(tmp_path, shell):
    exe = shutil.which(shell)
    if not exe:
        pytest.skip(f"{shell} not installed")
    if os.name == "nt" and subprocess.run([exe, "-c", "exit 0"]).returncode:
        pytest.skip(f"{shell} does not run here")
    out = tmp_path / "probe.txt"
    probe = tmp_path / "probe.py"
    probe.write_text(PROBE, encoding="utf-8")
    # POSIX paths for Git Bash on Windows
    posix = (
        (lambda p: subprocess.check_output([exe, "-c", f"cygpath -u '{p}'"], text=True).strip())
        if os.name == "nt"
        else str
    )
    run = f"'{posix(sys.executable)}' '{posix(probe)}' '{posix(out)}'"
    lines = [
        f"wrapped() {{ {run} wrapped; }}",
        f'hf() {{ {run} "$1"; }}',
        f"alias {KOREAN_HF}=hf",
        f". '{posix(SHELL_DIR / 'hook.sh')}'",
        "wrapped",
        "hf hf",
        f"{KOREAN_HF} alias",
        "exit",
    ]
    argv = [exe, "--norc", "-i"] if shell == "bash" else [exe, "-f", "-i"]
    screen = run_shell(tmp_path, argv, lines, pause=1.5)
    assert results(out) == {"wrapped": "False", "hf": "True", "alias": "True"}, screen
