
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
