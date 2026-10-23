"""`cdg term uninstall` puts every startup file back byte for byte and leaves nothing behind."""

import os
import time

import pytest
from typer.testing import CliRunner

from codigest.commands import term

BOM = b"\xef\xbb\xbf"


@pytest.fixture
def home(tmp_path, monkeypatch):
    """bash, zsh and one PowerShell profile under tmp_path; nothing else is touched."""
    monkeypatch.setattr(term.Path, "home", lambda: tmp_path)
    monkeypatch.setenv("CODIGEST_HOME", str(tmp_path / "data"))
    monkeypatch.setenv("TMPDIR", str(tmp_path / "tmp"))
    monkeypatch.setenv("TEMP", str(tmp_path / "tmp"))
    monkeypatch.delenv("ZDOTDIR", raising=False)
    profile = tmp_path / "Documents" / "PowerShell" / "Microsoft.PowerShell_profile.ps1"
    monkeypatch.setattr(term, "_powershell_profiles", lambda: [profile])
    monkeypatch.setattr(term.shutil, "which", lambda name: f"/bin/{name}")
    (tmp_path / "tmp").mkdir()
    return tmp_path


def run(*args):
    result = CliRunner().invoke(term.app, list(args))
    assert result.exit_code == 0, result.output
    return result.output


def snapshot(root):
    return {
        p.relative_to(root).as_posix(): p.read_bytes() if p.is_file() else None
        for p in sorted(root.rglob("*"))
    }


@pytest.mark.parametrize(
    "original",
    [
        b"alias g=git\n",
        b"alias g=git",  # no final newline
        b"alias g=git\n\n\n",  # trailing blank lines
        b"\n",
        b"",
        b"export A=1\r\nexport B=2\r\n",  # CRLF
        b"export A=1\r\nexport B=2",
        "export NAME='Ünï'\n".encode(),
    ],
)
def test_uninstall_restores_existing_rc_files_byte_for_byte(home, original):
    bashrc, zshrc = home / ".bashrc", home / ".zshrc"
    bashrc.write_bytes(original)
    zshrc.write_bytes(original)
    run("install", "--shell", "bash")
    run("install", "--shell", "zsh")
    assert term._BEGIN in bashrc.read_text(encoding="utf-8")
    run("uninstall")
    assert bashrc.read_bytes() == original
    assert zshrc.read_bytes() == original


@pytest.mark.parametrize("original", [b"Set-Alias g git\r\n", BOM + b"Set-Alias g git\r\n", b""])
def test_uninstall_restores_a_powershell_profile_with_or_without_bom(home, original):
    profile = term._powershell_profiles()[0]
    profile.parent.mkdir(parents=True)
    profile.write_bytes(original)
    run("install", "--shell", "powershell")
    # Windows PowerShell 5.1 needs the BOM to read a non-ASCII hook path.
    assert profile.read_bytes().startswith(BOM)
    run("uninstall")
    assert profile.read_bytes() == original


def test_install_then_uninstall_leaves_the_home_folder_as_it_was(home):
    (home / ".bashrc").write_bytes(b"alias g=git\n")
    before = snapshot(home)
    run("install", "--shell", "all")
    run("install", "--shell", "all")  # a second install changes nothing
    assert snapshot(home) != before
    run("uninstall")
    run("uninstall")  # nor does a second uninstall
    # Startup files that install created, their folders and the hook copies are gone.
    assert snapshot(home) == before


def test_repeated_installs_keep_one_block_and_identical_bytes(home):
    bashrc = home / ".bashrc"
    bashrc.write_bytes(b"alias g=git")
    run("install", "--shell", "bash")
    once = bashrc.read_bytes()
    run("install", "--shell", "bash")
    assert bashrc.read_bytes() == once and once.count(term._BEGIN.encode()) == 1


def test_lines_added_after_install_survive_uninstall(home):
    bashrc = home / ".bashrc"
    bashrc.write_bytes(b"alias g=git\n")
    run("install", "--shell", "bash")
    with bashrc.open("ab") as f:
        f.write(b"export LATER=1\n")
    run("uninstall")
    assert bashrc.read_bytes() == b"alias g=git\nexport LATER=1\n"


def test_uninstall_of_one_shell_keeps_the_other_and_the_hook_copies(home):
    run("install", "--shell", "all")
    run("uninstall", "--shell", "zsh")
    assert not (home / ".zshrc").exists()
    assert term._BEGIN in (home / ".bashrc").read_text(encoding="utf-8")
    assert (home / "data" / "shell" / "hook.sh").exists()


def test_a_block_from_an_older_install_is_removed_too(home):
    """Before uninstall restored bytes, install normalized the file and kept no record."""
    bashrc = home / ".bashrc"
    bashrc.write_bytes(f"alias g=git\n\n{term._BEGIN}\nif true; then :; fi\n{term._END}\n".encode())
    run("uninstall")
    assert bashrc.read_bytes() == b"alias g=git\n"


def test_uninstall_removes_stale_raw_captures_from_the_bash_hook(home):
    # Git Bash on Windows has uids of its own, such as 197609.
    raw = home / "tmp" / f"codigest-{os.getuid() if hasattr(os, 'getuid') else 197609}"
    raw.mkdir(mode=0o700)
    stale, live = raw / "1.1", raw / "2.2"
    stale.write_bytes(b"cmd\0unredacted output")
    live.write_bytes(b"cmd\0still running")
    day_ago = time.time() - 2 * 86400
    os.utime(stale, (day_ago, day_ago))
    run("install", "--shell", "bash")
    run("uninstall")
    assert not stale.exists() and live.exists()
    live.unlink()
    run("uninstall")
    assert not raw.exists()
