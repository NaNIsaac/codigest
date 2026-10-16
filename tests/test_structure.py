from codigest.core.structure import generate_ascii_tree


def test_sibling_with_shared_prefix_does_not_pull_in_directory(tmp_path):
    (tmp_path / ".git").mkdir()
    (tmp_path / "src").mkdir()
    (tmp_path / "src_old").mkdir()
    files = [tmp_path / ".gitignore", tmp_path / "src_old" / "a.py"]
    for f in files:
        f.write_text("")
    assert generate_ascii_tree(files, tmp_path) == "├── .gitignore\n└── src_old\n    └── a.py"
