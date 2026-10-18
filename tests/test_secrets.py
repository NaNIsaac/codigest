import subprocess

from typer.testing import CliRunner

from codigest.core import termstore
from codigest.core.redact import is_secret_file, redact, redact_code, redact_diff
from codigest.core.termstore import Entry
from codigest.main import app

GH_TOKEN = "ghp_abcdefghijklmnopqrstuvwxyz0123456789"
OPENAI_KEY = "sk-proj-abcdefghijklmnopqrstuvwx"


def test_credential_files_are_recognised_but_templates_are_not():
    for name in (".env", ".env.local", "id_rsa", "server.pem", "release.keystore", ".netrc"):
        assert is_secret_file(name), name
    for name in (".env.example", "env.py", "keys.py", "README.md"):
        assert not is_secret_file(name), name


def test_code_redaction_masks_tokens_but_leaves_ordinary_code_alone():
    code = f'TOKEN = "{GH_TOKEN}"\ntoken = get_token()\nURL = "postgres://admin:hunter2pass@db/x"\n'
    out = redact_code(code)
    assert GH_TOKEN not in out and "hunter2pass" not in out
    assert "token = get_token()" in out


def test_terminal_output_also_masks_key_value_pairs():
    assert "s3cr3tvalue" not in redact("password=s3cr3tvalue")
    assert "s3cr3tvalue" in redact_code("password=s3cr3tvalue")  # too ambiguous for source


def test_diff_drops_secret_files_and_masks_tokens_elsewhere():
    diff = (
        "diff --git a/.env b/.env\n--- a/.env\n+++ b/.env\n-KEY=old\n+KEY=new\n"
        f"diff --git a/app.py b/app.py\n--- a/app.py\n+++ b/app.py\n+T = '{GH_TOKEN}'\n"
    )
    out = redact_diff(diff)
    assert "KEY=new" not in out and "contents withheld: .env" in out
    assert "diff --git a/app.py b/app.py" in out and GH_TOKEN not in out


def test_captures_are_redacted_before_they_are_stored(tmp_path, monkeypatch):
    monkeypatch.setenv("CODIGEST_HOME", str(tmp_path))
    termstore.append_entry(Entry(command="python deploy.py", output=f"using {OPENAI_KEY}", exit_code=1))
    assert OPENAI_KEY not in termstore.get_history_path().read_text(encoding="utf-8")


def _repo(tmp_path):
    (tmp_path / ".env").write_text(f"OPENAI_API_KEY={OPENAI_KEY}\n")
    (tmp_path / "config.py").write_text(f'TOKEN = "{GH_TOKEN}"\n')
    git = ["git", "-c", "user.name=t", "-c", "user.email=t@t"]
    subprocess.run([*git, "init", "-q"], cwd=tmp_path, check=True)
    subprocess.run([*git, "add", "."], cwd=tmp_path, check=True)
    subprocess.run([*git, "commit", "-qm", "init"], cwd=tmp_path, check=True)
    (tmp_path / ".env").write_text("OPENAI_API_KEY=sk-proj-zzzzzzzzzzzzzzzzzzzzzzzz\n")
    (tmp_path / "config.py").write_text(f'TOKEN = "{GH_TOKEN}"\nDEBUG = True\n')


def test_scan_and_pack_never_print_secrets(tmp_path, monkeypatch):
    _repo(tmp_path)
    monkeypatch.chdir(tmp_path)
    monkeypatch.setenv("CODIGEST_HOME", str(tmp_path / "home"))
    runner = CliRunner()

    scan = runner.invoke(app, ["scan", "-s", "--no-copy", "-y"]).output
    assert "Withheld (may contain secrets): .env" in scan
    pack = runner.invoke(app, ["pack", "-s", "--no-copy"]).output
    assert "contents withheld: .env" in pack and ".env       may contain secrets" in pack

    for out in (scan, pack):
        assert OPENAI_KEY not in out and "sk-proj-zzz" not in out and GH_TOKEN not in out
