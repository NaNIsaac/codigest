"""The capture hooks in an interactive shell on a pseudo-terminal.

A line the hooks capture gives the programs it runs a pipe for stdout. A runner (a program that
runs other command lines, like hanfix's hf) lists itself in CODIGEST_RUNNERS: its own line keeps
the terminal, and each line it hands to __cdg_run is captured or not by the hook's own rules.
"""

import json
import os
import shutil
import subprocess
import sys
import threading
import time
from pathlib import Path

import pytest

SHELL_DIR = Path(__file__).parents[1] / "src" / "codigest" / "shell"
PROBE = """import os, sys
print("out-" + sys.argv[2], flush=True)
with open(sys.argv[1], "a") as f:
    f.write(f"{sys.argv[2]} {os.isatty(1)}\\n")
"""
# Stands in for `cdg term ingest`: one JSON line per captured command.
FAKE_CDG = """import json, sys
args = sys.argv[1:]
files = [a.split("=", 1)[1] for a in args if a.startswith("--file=")]
if files:
    command, _, output = open(files[0], "rb").read().decode().partition("\\0")
else:
    record = json.loads(sys.stdin.buffer.read().decode("utf-8-sig"))
    command, output = record["command"], record["output"]
with open(sys.argv[0] + ".log", "a", encoding="utf-8") as f:
    f.write(json.dumps({"command": command, "output": output}) + "\\n")
"""
WINDOWS = os.name == "nt"


class Pty:
    """An interactive program on a pseudo-terminal: stdlib pty on POSIX, ConPTY on Windows."""

    def __init__(self, argv, env):
        self.output = []
        if WINDOWS:
            winpty = pytest.importorskip("winpty", reason="pywinpty is needed for a ConPTY")
            self.proc = winpty.PtyProcess.spawn(argv, env=env, dimensions=(50, 250))
            self._write = self.proc.write
            self._read = lambda: self.proc.read(4096)
        else:
            import fcntl
            import pty
            import struct
            import termios

            pid, self.fd = pty.fork()
            if pid == 0:
                os.execvpe(argv[0], argv, env)
            self.pid = pid
            fcntl.ioctl(self.fd, termios.TIOCSWINSZ, struct.pack("HHHH", 50, 250, 0, 0))
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
            # PSReadLine on POSIX waits for the cursor position report a terminal sends; ConPTY answers itself
            if not WINDOWS:
                for _ in range(chunk.count("\x1b[6n")):
                    self._write("\x1b[1;1R")

    def type_lines(self, lines, pause):
        for line in lines:
            time.sleep(pause)
            self._write(line + "\r")

    def wait(self, timeout):
        deadline = time.time() + timeout
        if WINDOWS:
            while self.proc.isalive() and time.time() < deadline:
                time.sleep(0.2)
            self.proc.terminate(force=True)
            return
        while time.time() < deadline:
            if os.waitpid(self.pid, os.WNOHANG)[0]:
                return
            time.sleep(0.2)
        os.kill(self.pid, 9)
        os.waitpid(self.pid, 0)


class Session:
    """A sandbox: fake cdg on PATH, the probe, and what both recorded."""

    def __init__(self, tmp_path, to_path=str):
        self.tmp = tmp_path
        self.to_path = to_path
        bin_dir = tmp_path / "bin"
        bin_dir.mkdir()
        fake = bin_dir / "fake_cdg.py"
        fake.write_text(FAKE_CDG, encoding="utf-8")
        self.ingest_log = Path(f"{fake}.log")
        # The hooks only load when cdg is on PATH.
        script = bin_dir / "cdg"
        script.write_text(
            f'#!/bin/sh\nexec "{to_path(sys.executable)}" "{to_path(fake)}" "$@"\n',
            encoding="utf-8",
        )
        script.chmod(0o755)
        (bin_dir / "cdg.cmd").write_text(f'@"{sys.executable}" "{fake}" %*\r\n', encoding="utf-8")
        self.probe_out = tmp_path / "probe.txt"
        probe = tmp_path / "probe.py"
        probe.write_text(PROBE, encoding="utf-8")
        self.probe = f"'{to_path(sys.executable)}' '{to_path(probe)}' '{to_path(self.probe_out)}'"
        self.env = dict(os.environ)
        self.env.update(
            PATH=f"{bin_dir}{os.pathsep}{self.env['PATH']}",
            HOME=str(tmp_path),
            TERM="xterm-256color",
            CODIGEST_CAP_SKIP="keeptty",
        )
        for name in ("CODIGEST_NOCAP", "CODIGEST_RUNNERS", "CODIGEST_HOOK", "TERM_PROGRAM"):
            self.env.pop(name, None)

    def run(self, argv, lines, pause):
        term = Pty(argv, self.env)
        term.type_lines(lines, pause)
        term.wait(40)
        time.sleep(1)  # ingest runs in the background
        return "".join(term.output)

    def ttys(self):
        words = (
            self.probe_out.read_text(encoding="utf-8").split() if self.probe_out.exists() else []
        )
        return dict(zip(words[::2], words[1::2], strict=True))

    def captured(self):
        if not self.ingest_log.exists():
            return []
        return [
            json.loads(line) for line in self.ingest_log.read_text(encoding="utf-8").splitlines()
        ]


def need(exe):
    path = shutil.which(exe)
    if not path:
        pytest.skip(f"{exe} not installed")
    return path


def vscode_script(shell):
    code = shutil.which("code")
    if not code:
        pytest.skip("VS Code not installed")
    out = subprocess.run(
        [code, "--locate-shell-integration-path", shell],
        check=False,
        capture_output=True,
        text=True,
        shell=WINDOWS,
    )
    path = Path(out.stdout.strip())
    if not path.is_file():
        pytest.skip(f"VS Code has no shell integration script for {shell}")
    return path


def reported_lines(screen):
    """The command lines a terminal's shell integration reported (OSC 633 ; E ; <line> [; nonce])."""
    return [part.split("\x07")[0].split(";")[0] for part in screen.split("\x1b]633;E;")[1:]]


def pwsh_argv(pwsh, s):
    # Set before the first prompt, so PSReadLine neither reads nor writes the user's history file.
    setup = (
        f"Set-PSReadLineOption -HistorySavePath '{s.tmp / 'history.txt'}' -HistorySaveStyle SaveNothing "
        "-PredictionSource History"
    )
    return [pwsh, "-NoProfile", "-NoLogo", "-NoExit", "-Command", setup]


def pwsh_lines(s, hook_first, hook_times=1):
    probe = f"& {s.probe}"
    runner = [
        f'function runit {{ {probe} runner; . __cdg_run "{probe} keeptty"; . __cdg_run "{probe} captured" }}',
        "$global:CODIGEST_RUNNERS += 'runit'",
    ]
    hook = [f". '{SHELL_DIR / 'hook.ps1'}'"] * hook_times
    return [
        f"function wrapped {{ {probe} wrapped }}",
        *(hook + runner if hook_first else runner + hook),
        "wrapped",
        "runit",
    ]


def history_lines(s):
    hist = s.tmp / "hist.txt"
    return hist, [
        f"[Microsoft.PowerShell.PSConsoleReadLine]::GetHistoryItems().CommandLine | Set-Content '{hist}'",
        f"(Get-History).CommandLine | Add-Content '{hist}'",
        f"\"prev=$($null -eq $global:__cdgPrev)\" | Add-Content '{hist}'",
    ]


ALL_TTYS = {"wrapped": "False", "runner": "True", "keeptty": "True", "captured": "False"}


@pytest.mark.parametrize("hook_first", [True, False], ids=["hook-then-runner", "runner-then-hook"])
def test_pwsh_runner_keeps_the_terminal_and_hands_lines_to_cdg_run(tmp_path, hook_first):
    pwsh = need("pwsh")
    s = Session(tmp_path)
    hist, report = history_lines(s)
    screen = s.run(pwsh_argv(pwsh, s), [*pwsh_lines(s, hook_first), *report, "exit"], pause=2)
    assert s.ttys() == ALL_TTYS, screen[-2000:]
    commands = [c["command"] for c in s.captured()]
    assert "wrapped" in commands and f"& {s.probe} captured" in commands, commands
    assert "runit" not in commands and f"& {s.probe} keeptty" not in commands, commands
    assert any("out-captured" in c["output"] for c in s.captured())
    lines = hist.read_text(encoding="utf-8").splitlines()
    # PSReadLine and the engine history both keep the lines as typed, once each
    assert lines.count("wrapped") == 2 and lines.count("runit") == 2, lines
    assert not any(line.startswith(". __cdg") for line in lines), lines
    assert lines[-1] == "prev=True", lines


def test_pwsh_hook_loaded_twice_chains_nothing(tmp_path):
    pwsh = need("pwsh")
    s = Session(tmp_path)
    hist, report = history_lines(s)
    s.run(pwsh_argv(pwsh, s), [*pwsh_lines(s, True, hook_times=2), *report, "exit"], pause=2)
    assert s.ttys() == ALL_TTYS
    assert [c["command"] for c in s.captured()].count("wrapped") == 1
    lines = hist.read_text(encoding="utf-8").splitlines()
    assert lines.count("wrapped") == 2 and lines[-1] == "prev=True", lines


def test_pwsh_readline_replaced_later_without_calling_codigest(tmp_path):
    pwsh = need("pwsh")
    s = Session(tmp_path)
    hist, report = history_lines(s)
    *setup, wrapped, runit = pwsh_lines(s, True)
    # atuin's PSConsoleHostReadLine reads the line itself and never calls the one it replaced
    other = (
        "function global:PSConsoleHostReadLine { "
        "[Microsoft.PowerShell.PSConsoleReadLine]::ReadLine($host.Runspace, $ExecutionContext, $true) }"
    )
    lines = [*setup, other, "$null", wrapped, runit, *report, "exit"]
    screen = s.run(pwsh_argv(pwsh, s), lines, pause=2)
    assert s.ttys() == ALL_TTYS, screen[-2000:]
    assert "wrapped" in [c["command"] for c in s.captured()]
    lines = hist.read_text(encoding="utf-8").splitlines()
    assert not any(line.startswith(". __cdg") for line in lines), lines
    assert lines.count("wrapped") == 2 and lines[-1] == "prev=False", lines


def test_pwsh_vscode_integration_sees_the_typed_line(tmp_path):
    pwsh = need("pwsh")
    script = vscode_script("pwsh")
    s = Session(tmp_path)
    lines = [
        f"function wrapped {{ & {s.probe} wrapped }}",
        f". '{SHELL_DIR / 'hook.ps1'}'",
        f". '{script}'",  # VS Code loads its integration after the profile
        "$null",  # the first line after it puts codigest back on top, uncaptured
        "wrapped",
        "exit",
    ]
    screen = s.run(pwsh_argv(pwsh, s), lines, pause=2)
    reported = reported_lines(screen)
    assert "wrapped" in reported, reported
    assert not any(r.startswith(". __cdg") for r in reported), reported
    assert s.ttys() == {"wrapped": "False"}
    assert "wrapped" in [c["command"] for c in s.captured()]


def posix_shell(shell):
    exe = need(shell)
    if (
        WINDOWS
        and subprocess.run([exe, "-c", "exit 0"], check=False, capture_output=True).returncode
    ):
        pytest.skip(f"{shell} does not run here")
    to_path = str
    if WINDOWS:  # Git Bash

        def to_path(p):
            return subprocess.check_output([exe, "-c", f"cygpath -u '{p}'"], text=True).strip()

    argv = [exe, "--norc", "-i"] if shell == "bash" else [exe, "-f", "-i"]
    return argv, to_path


@pytest.mark.parametrize("hook_first", [True, False], ids=["hook-then-runner", "runner-then-hook"])
@pytest.mark.parametrize("shell", ["bash", "zsh"])
def test_sh_runner_keeps_the_terminal_and_hands_lines_to_cdg_run(tmp_path, shell, hook_first):
    argv, to_path = posix_shell(shell)
    s = Session(tmp_path, to_path)
    runner = [
        f'runit() {{ {s.probe} runner; __cdg_run "{s.probe} keeptty"; __cdg_run "{s.probe} captured"; }}',
        'CODIGEST_RUNNERS="${CODIGEST_RUNNERS-} runit"',
    ]
    hook = [f". '{to_path(SHELL_DIR / 'hook.sh')}'"]
    jobs = tmp_path / "jobs.txt"
    lines = [
        f"wrapped() {{ {s.probe} wrapped; }}",
        *(hook + runner if hook_first else runner + hook),
        "wrapped",
        "runit",
        # the capture __cdg_run opened inside a function left no job behind, and the shell goes on
        f"jobs > '{to_path(jobs)}'",
        f"{s.probe} after",
        "exit",
    ]
    screen = s.run(argv, lines, pause=1.5)
    assert s.ttys() == {**ALL_TTYS, "after": "False"}, screen[-2000:]
    records = {c["command"]: c["output"] for c in s.captured()}
    assert "out-captured" in records.get(f"{s.probe} captured", ""), records
    assert "runit" not in records and f"{s.probe} keeptty" not in records, records
    assert jobs.read_text(encoding="utf-8").strip() == ""


@pytest.mark.xfail(
    strict=True, reason="#52: VS Code runs the chained DEBUG trap only at the Enter handler"
)
def test_bash_vscode_integration_sees_the_typed_line(tmp_path):
    argv, to_path = posix_shell("bash")
    script = vscode_script("bash")
    s = Session(tmp_path, to_path)
    lines = [
        f"wrapped() {{ {s.probe} wrapped; }}",
        f". '{to_path(SHELL_DIR / 'hook.sh')}'",
        f". '{to_path(script)}'",
        "true",
        "wrapped",
        "exit",
    ]
    screen = s.run(argv, lines, pause=1.5)
    assert "wrapped" in reported_lines(screen), reported_lines(screen)
    assert s.ttys() == {"wrapped": "False"}
    assert "wrapped" in [c["command"] for c in s.captured()]


# A rewriter turns the typed line `<probe> <tag>` into another one; each tag tests one rule.
REWRITTEN_TTYS = {
    "fixed": "False",
    "keeptty": "True",
    "asked": "False",
    "boom": "False",
    "slow": "False",
    "bad": "False",
    "noisy": "False",
}
REWRITER_TAGS = ["fixme", "tofix-tty", "askme", "", "askno", "n", "boom", "slow", "bad", "noisy"]


def rewriter_lines(prefix):
    # askme is answered with Enter, askno with `n` (its Enter then runs an empty line)
    return [tag if tag in ("", "n") else f"{prefix} {tag}" for tag in REWRITER_TAGS]


PWSH_REWRITER = """function global:rw([string]$l) {
    switch -regex ($l) {
        'fixme$' { "__P__ fixed" }
        'tofix-tty$' { "__P__ keeptty" }
        'askme$' { @{ Line = "__P__ asked"; Confirm = $true } }
        'askno$' { @{ Line = "__P__ refused"; Confirm = $true } }
        'boom$' { throw 'rewriter-boom' }
        'slow$' { Start-Sleep -Milliseconds 300; "__P__ late" }
        'bad$' { 'if (' }
        'noisy$' { Write-Error 'rewriter-noise'; "__P__ late" }
    }
}
$global:CODIGEST_REWRITERS += 'rw'
"""


@pytest.mark.parametrize(
    "hook_first", [True, False], ids=["hook-then-rewriter", "rewriter-then-hook"]
)
def test_pwsh_rewriter_replaces_the_line_before_it_runs(tmp_path, hook_first):
    pwsh = need("pwsh")
    s = Session(tmp_path)
    probe = f"& {s.probe}"
    # From a file, so its messages are not on the screen as typed text
    script = tmp_path / "rw.ps1"
    script.write_text(PWSH_REWRITER.replace("__P__", probe), encoding="utf-8")
    hook, rw = f". '{SHELL_DIR / 'hook.ps1'}'", f". '{script}'"
    hist, report = history_lines(s)
    lines = [*([hook, rw] if hook_first else [rw, hook]), *rewriter_lines(probe), *report, "exit"]
    screen = s.run(pwsh_argv(pwsh, s), lines, pause=2)
    assert s.ttys() == REWRITTEN_TTYS, screen[-2000:]
    records = {c["command"]: c["output"] for c in s.captured()}
    assert "out-fixed" in records.get(f"{probe} fixed", ""), records
    assert f"{probe} keeptty" not in records and f"{probe} fixme" not in records, records
    assert "rewriter-boom" not in screen and "rewriter-noise" not in screen
    lines = hist.read_text(encoding="utf-8").splitlines()
    # PSReadLine and the engine history hold the line that ran, once each, and not the typed one
    assert lines.count(f"{probe} fixed") == 2 and lines.count(f"{probe} asked") == 2, lines
    assert lines.count(f"{probe} boom") == 2, lines
    assert not any(line.endswith(("fixme", "askme", "askno", "refused")) for line in lines), lines


SH_REWRITER = """rw() {
    case $1 in
        *' fixme') REPLY="$P fixed" ;;
        *' tofix-tty') REPLY="$P keeptty" ;;
        *' askme') REPLY="$P asked"; return 2 ;;
        *' askno') REPLY="$P refused"; return 2 ;;
        *' boom') echo rewriter-boom >&2; return 7 ;;
        *' slow') sleep 0.3; REPLY="$P late" ;;
        *' bad') REPLY='if (' ;;
        *' noisy') echo rewriter-noise; return 1 ;;
        *) return 1 ;;
    esac
}
CODIGEST_REWRITERS="${CODIGEST_REWRITERS-} rw"
"""


@pytest.mark.parametrize(
    "hook_first", [True, False], ids=["hook-then-rewriter", "rewriter-then-hook"]
)
@pytest.mark.parametrize("shell", ["bash", "zsh"])
def test_sh_rewriter_replaces_the_line_before_it_runs(tmp_path, shell, hook_first):
    argv, to_path = posix_shell(shell)
    s = Session(tmp_path, to_path)
    script = tmp_path / "rw.sh"
    script.write_text(f'P="{s.probe}"\n{SH_REWRITER}', encoding="utf-8", newline="\n")
    hook, rw = f". '{to_path(SHELL_DIR / 'hook.sh')}'", f". '{to_path(script)}'"
    hist = tmp_path / "hist.txt"
    lines = [
        *([hook, rw] if hook_first else [rw, hook]),
        "true",
        *rewriter_lines(s.probe),
        f"fc -ln 1 > '{to_path(hist)}'",
        "exit",
    ]
    screen = s.run(argv, lines, pause=1.5)
    assert s.ttys() == REWRITTEN_TTYS, screen[-2000:]
    records = {c["command"]: c["output"] for c in s.captured()}
    assert "out-fixed" in records.get(f"{s.probe} fixed", ""), records
    assert f"{s.probe} keeptty" not in records and f"{s.probe} fixme" not in records, records
    assert "rewriter-boom" not in screen and "rewriter-noise" not in screen
    history = [line.strip() for line in hist.read_text(encoding="utf-8").splitlines()]
    assert history.count(f"{s.probe} fixed") == 1 and history.count(f"{s.probe} asked") == 1, (
        history
    )
    assert not any(line.endswith(("fixme", "askme", "askno", "refused")) for line in history), (
        history
    )


def test_pwsh_captured_line_sees_the_previous_status(tmp_path):
    """#55: the wrapper's own statements used to reset $? before the typed line ran."""
    pwsh = need("pwsh")
    s = Session(tmp_path)
    state = tmp_path / "state.txt"
    fail = 'pwsh -NoProfile -Command "exit {}"'
    lines = [
        f". '{SHELL_DIR / 'hook.ps1'}'",
        # a rewriter that turns `failme` into a failing command, as hanfix's hf does with a fix
        f"function global:rw([string]$l) {{ if ($l -eq 'failme') {{ '{fail.format(4)}' }} }}",
        "$global:CODIGEST_REWRITERS += 'rw'",
        fail.format(3),
        f"\"typed $? $LASTEXITCODE\" | Add-Content '{state}'",
        f"\"next $?\" | Add-Content '{state}'",
        "failme",
        f"\"rewritten $? $LASTEXITCODE\" | Add-Content '{state}'",
        "exit",
    ]
    screen = s.run(pwsh_argv(pwsh, s), lines, pause=2)
    assert state.read_text(encoding="utf-8").splitlines() == [
        "typed False 3",
        "next True",
        "rewritten False 4",
    ], screen[-2000:]
    commands = [c["command"] for c in s.captured()]
    assert fail.format(4) in commands and "failme" not in commands, commands
