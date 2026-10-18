
from codigest.commands import pack
from codigest.core import resolver
from codigest.core.termstore import Entry


def test_failure_is_dropped_once_the_same_command_passes(tmp_path):
    from datetime import datetime, timedelta

    def at(minutes_ago):
        return (datetime.now().astimezone() - timedelta(minutes=minutes_ago)).isoformat()

    cwd = str(tmp_path)
    other = Entry(command="mypy .", exit_code=2, cwd=cwd, started_at=at(4))
    fail = Entry(command="pytest", exit_code=1, cwd=cwd, started_at=at(3))
    passed = Entry(command="pytest", exit_code=0, cwd=cwd, started_at=at(2))
    elsewhere = Entry(command="make", exit_code=1, cwd=str(tmp_path.parent), started_at=at(1))

    assert pack.pick_failure([other, fail], tmp_path) is fail
    assert pack.pick_failure([other, fail, passed, elsewhere], tmp_path) is other
    stale = Entry(command="ruff check", exit_code=1, cwd=cwd, started_at=at(90))
    assert pack.pick_failure([stale], tmp_path) is None


def test_referenced_files_reads_python_and_compiler_locations(tmp_path):
    a, b = tmp_path / "a.py", tmp_path / "src" / "b.ts"
    output = f'File "{a}", line 7, in f\nsrc/b.ts(3,5): error\n/usr/lib/python3/os.py:12: nope'
    found = pack.referenced_files(output, [tmp_path], {a, b})
    assert found == {a: [7], b: [3]}


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
    found = {p.relative_to(tmp_path).as_posix() for p in resolver.js_imports(main)}
    assert found == {"price.ts", "lib/index.ts", "old.cjs", "app.service.ts"}


def test_excerpt_merges_nearby_references_and_skips_short_files(tmp_path):
    long = tmp_path / "long.py"
    long.write_text("".join(f"x{i} = {i}\n" for i in range(1, 201)))
    text, shown = pack.excerpt(long, [50, 55, 150])
    assert shown == "40-65,140-160"
    assert text.splitlines()[0] == " 40: x40 = 40" and "\n...\n" in text

    short = tmp_path / "short.py"
    short.write_text("a = 1\n" * 30)
    assert pack.excerpt(short, [3]) is None


def test_scan_resolve_follows_ts_imports_recursively(tmp_path, monkeypatch):
    from typer.testing import CliRunner

    from codigest.main import app

    (tmp_path / ".git").mkdir()  # project root marker
    (tmp_path / "src").mkdir()
    (tmp_path / "src" / "main.ts").write_text("import { p } from './prices.js'\n")
    (tmp_path / "src" / "prices.ts").write_text("import { RATE } from './rate'\nexport const p = 1\n")
    (tmp_path / "src" / "rate.ts").write_text("export const RATE = 0.1\n")
    (tmp_path / "src" / "unrelated.ts").write_text("export const X = 1\n")
    monkeypatch.chdir(tmp_path)
    out = CliRunner().invoke(app, ["scan", "src/main.ts", "-r", "-s", "--no-copy", "-y"]).output
    assert all(f'path="src/{name}"' in out for name in ("main.ts", "prices.ts", "rate.ts"))
    assert "unrelated.ts" not in out
