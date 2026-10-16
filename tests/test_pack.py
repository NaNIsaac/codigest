
from codigest.commands import pack
from codigest.core.termstore import Entry


def test_failure_is_dropped_once_the_same_command_passes(tmp_path):
    cwd = str(tmp_path)
    fail = Entry(command="pytest", exit_code=1, cwd=cwd, started_at="1")
    other = Entry(command="mypy .", exit_code=2, cwd=cwd, started_at="0")
    passed = Entry(command="pytest", exit_code=0, cwd=cwd, started_at="2")
    elsewhere = Entry(command="make", exit_code=1, cwd=str(tmp_path.parent), started_at="3")

    assert pack.pick_failure([other, fail], tmp_path) is fail
    assert pack.pick_failure([other, fail, passed, elsewhere], tmp_path) is other


def test_referenced_files_reads_python_and_compiler_locations(tmp_path):
    a, b = tmp_path / "a.py", tmp_path / "src" / "b.ts"
    output = f'File "{a}", line 7, in f\nsrc/b.ts(3,5): error\n/usr/lib/python3/os.py:12: nope'
    found = pack.referenced_files(output, [tmp_path], {a, b})
    assert found == {a: 7, b: 3}


def test_changed_paths_skips_deleted_files(tmp_path):
    (tmp_path / "kept.py").write_text("")
    diff = "diff --git a/kept.py b/kept.py\ndiff --git a/gone.py b/gone.py\n"
    assert pack.changed_paths(diff, tmp_path) == [(tmp_path / "kept.py").resolve()]


def test_pack_adds_the_test_that_imports_a_changed_module(tmp_path, monkeypatch):
    import subprocess

    from typer.testing import CliRunner

    from codigest.main import app

    (tmp_path / "shop").mkdir()
    (tmp_path / "tests").mkdir()
    (tmp_path / "shop" / "prices.py").write_text("RATE = 1\n")
    (tmp_path / "tests" / "test_prices.py").write_text("from shop.prices import RATE\n")
    (tmp_path / "tests" / "test_other.py").write_text("X = 1\n")
    git = ["git", "-c", "user.name=t", "-c", "user.email=t@t"]
    subprocess.run([*git, "init", "-q"], cwd=tmp_path, check=True)
    subprocess.run([*git, "add", "."], cwd=tmp_path, check=True)
    subprocess.run([*git, "commit", "-qm", "init"], cwd=tmp_path, check=True)
    (tmp_path / "shop" / "prices.py").write_text("RATE = 2\n")

    monkeypatch.chdir(tmp_path)
    monkeypatch.setenv("CODIGEST_HOME", str(tmp_path / "home"))
    out = CliRunner().invoke(app, ["pack", "-s", "--no-copy"]).output
    assert 'path="tests/test_prices.py" reason="tests shop/prices.py"' in out
    assert "test_other.py" not in out


def test_js_imports_resolve_like_tsc(tmp_path):
    (tmp_path / "lib").mkdir()
    for name in ("price.ts", "lib/index.ts", "old.cjs", "app.service.ts"):
        (tmp_path / name).write_text("")
    main = tmp_path / "main.ts"
    main.write_text(
        "import { a } from './price.js'\n"
        "export * from \"./lib\"\n"
        "const o = require('./old.cjs')\n"
        "import './app.service'\n"
        "import React from 'react'\n"
    )
    found = {p.relative_to(tmp_path).as_posix() for p in pack.js_imports(main)}
    assert found == {"price.ts", "lib/index.ts", "old.cjs", "app.service.ts"}
