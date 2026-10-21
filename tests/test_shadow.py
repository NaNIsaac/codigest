from codigest.core.shadow import ContextAnchor


def test_second_update_moves_the_baseline(tmp_path):
    src = tmp_path / "a.py"
    src.write_text("x = 1\n", encoding="utf-8")
    anchor = ContextAnchor(tmp_path)
    anchor.update([src])

    src.write_text("x = 2\n", encoding="utf-8")
    anchor.update([src])

    assert anchor.read_anchor_file(src.relative_to(tmp_path)) == "x = 2\n"
    assert anchor.get_changes([src]) == ""


import pytest


def _project(root, files):
    for rel, text in files.items():
        path = root / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")


def _scan(root):
    return sorted(
        p for p in root.rglob("*")
        if p.is_file() and not {".git", ".codigest"} & set(p.relative_to(root).parts)
    )


def test_git_named_files_show_up_in_the_diff(tmp_path):
    names = [".gitignore", ".github/workflows/ci.yml", ".gitattributes", ".gitkeep", "notes.github.md"]
    _project(tmp_path, {"a.py": "x = 1\n", **{n: "one\n" for n in names}})
    anchor = ContextAnchor(tmp_path)
    anchor.update(_scan(tmp_path))

    _project(tmp_path, {n: "two\n" for n in names})
    changes = anchor.get_changes(_scan(tmp_path))

    for name in names:
        assert f"diff --git a/{name} b/{name}" in changes


def test_changed_files_keep_spaces_and_non_ascii_names(tmp_path):
    names = ["my dir/c.md", "한글 파일.txt", "a b.py", ".github/ci.yml"]
    _project(tmp_path, {n: "one\n" for n in names})
    anchor = ContextAnchor(tmp_path)
    anchor.update(_scan(tmp_path))

    _project(tmp_path, {n: "two\n" for n in names})

    assert anchor.get_changed_files(_scan(tmp_path)) == sorted(anchor.root / n for n in names)


@pytest.mark.parametrize(
    "line, expected",
    [
        ("diff --git a/a.py b/a.py", ("a.py", "a.py")),
        ("diff --git a/my dir/c.md b/my dir/c.md", ("my dir/c.md", "my dir/c.md")),
        (r'diff --git "a/\355\225\234.txt" "b/\355\225\234.txt"', ("한.txt", "한.txt")),
        (r'diff --git "a/tab\there" "b/tab\there"', ("tab\there", "tab\there")),
        (r'diff --git "a/q\"uote" "b/q\"uote"', ('q"uote', 'q"uote')),
        (r'diff --git "a/back\\slash" "b/back\\slash"', ("back\\slash", "back\\slash")),
        ("diff --git a/old name.py b/new name.py", ("old name.py", "new name.py")),
        (r'diff --git a/plain.py "b/\355\225\234.py"', ("plain.py", "한.py")),
        ("--- a/a.py", None),
        ("diff --git a/only-one", None),
    ],
)
def test_diff_headers_are_parsed_like_git_writes_them(line, expected):
    from codigest.core.shadow import diff_header_paths

    assert diff_header_paths(line) == expected


def test_files_outside_the_scan_are_not_reported_as_deleted(tmp_path):
    """A file that still exists but is no longer scanned (a new ignore rule, a narrower
    filter) is not a deletion; one that is really gone still is."""
    _project(tmp_path, {"a.py": "x = 1\n", "notes.log": "log\n"})
    anchor = ContextAnchor(tmp_path)
    anchor.update(_scan(tmp_path))
    kept = [tmp_path / "a.py"]

    assert anchor.get_changes(kept) == ""

    (tmp_path / "notes.log").unlink()
    changes = anchor.get_changes(kept)
    assert "deleted file mode" in changes and "notes.log" in changes
