"""What pack shows about the code itself: the function a failing test calls, and only the
changed parts of a changed file."""

import subprocess
from datetime import datetime

from typer.testing import CliRunner

from codigest.commands import pack
from codigest.core import termstore
from codigest.main import app

PADDING = "".join(f"def unrelated_{i}():\n    return {i}\n\n\n" for i in range(30))


def repo(tmp_path, files):
    for name, text in files.items():
        (tmp_path / name).parent.mkdir(parents=True, exist_ok=True)
        (tmp_path / name).write_text(text)
    git = ["git", "-c", "user.name=t", "-c", "user.email=t@t"]
    subprocess.run([*git, "init", "-q"], cwd=tmp_path, check=True)
    subprocess.run([*git, "add", "."], cwd=tmp_path, check=True)
    subprocess.run([*git, "commit", "-qm", "init"], cwd=tmp_path, check=True)


def run_pack(tmp_path, monkeypatch, failing_output=None):
    monkeypatch.chdir(tmp_path)
    monkeypatch.setenv("CODIGEST_HOME", str(tmp_path / "home"))
    if failing_output:
        termstore.append_entry(
            termstore.Entry(
                command="pytest -q",
                output=failing_output,
                exit_code=1,
                cwd=str(tmp_path),
                started_at=datetime.now().astimezone().isoformat(),
            )
        )
    return CliRunner().invoke(app, ["pack"]).stdout


def test_a_committed_bug_brings_the_function_the_failing_test_calls(tmp_path, monkeypatch):
    repo(
        tmp_path,
        {
            "shop/__init__.py": "from .prices import total\n",
            "shop/prices.py": PADDING + "def total(items):\n    return sum(items) * 2\n",
            "tests/test_prices.py": "from shop import total\n\n\ndef test_total():\n    assert total([1, 2]) == 3\n",
        },
    )

    out = run_pack(tmp_path, monkeypatch, "tests/test_prices.py:5: AssertionError\n1 failed")

    assert 'path="shop/prices.py" reason="called from failing test tests/test_prices.py"' in out
    assert "return sum(items) * 2" in out
    assert "unrelated_3" not in out


def test_a_changed_long_file_comes_as_its_hunks(tmp_path, monkeypatch):
    repo(tmp_path, {"shop/prices.py": PADDING + "def total(items):\n    return sum(items)\n"})
    path = tmp_path / "shop" / "prices.py"
    path.write_text(path.read_text().replace("return sum(items)", "return sum(items) * 2"))

    out = run_pack(tmp_path, monkeypatch)

    block = out[out.index('<file path="shop/prices.py"') :]
    assert 'reason="changed" lines="' in block
    assert "return sum(items) * 2" in block
    assert "unrelated_3()" not in block


def test_hunk_spans_reads_the_new_side(tmp_path):
    diff = (
        "diff --git a/x.py b/x.py\n--- a/x.py\n+++ b/x.py\n"
        "@@ -10,3 +12,4 @@\n a\n+b\n c\n d\n@@ -40 +43 @@\n-e\n+f\n"
    )
    assert pack.hunk_spans(diff, tmp_path) == {(tmp_path / "x.py").resolve(): [(12, 15), (43, 43)]}


def test_long_output_keeps_the_first_failures_and_the_summary():
    text = "\n".join(f"line {i}" for i in range(500))

    short = pack.shorten_output(text).splitlines()

    assert short[0] == "line 0" and short[-1] == "line 499"
    assert "380 lines left out" in short[pack.OUTPUT_HEAD]
    assert pack.shorten_output("a\nb") == "a\nb"
