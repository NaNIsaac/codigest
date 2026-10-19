"""What a script or an agent's shell sees: output on a pipe, code verbatim, outlines it can jump from."""
import pyperclip
from typer.testing import CliRunner

from codigest.core import outline, tags
from codigest.main import app


def project(tmp_path, monkeypatch, files):
    (tmp_path / ".git").mkdir()
    for name, text in files.items():
        path = tmp_path / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")
    monkeypatch.chdir(tmp_path)


def test_a_pipe_gets_the_output_and_the_clipboard_is_left_alone(tmp_path, monkeypatch):
    project(tmp_path, monkeypatch, {"a.py": "x = 1\n"})
    copied = []
    monkeypatch.setattr(pyperclip, "copy", copied.append)

    result = CliRunner().invoke(app, ["scan"])

    assert '<file path="a.py">' in result.stdout
    assert copied == []


def test_copy_can_still_be_asked_for_on_a_pipe(tmp_path, monkeypatch):
    project(tmp_path, monkeypatch, {"a.py": "x = 1\n"})
    copied = []
    monkeypatch.setattr(pyperclip, "copy", copied.append)

    CliRunner().invoke(app, ["scan", "--copy"])

    assert len(copied) == 1


def test_code_is_shared_exactly_as_written(tmp_path, monkeypatch):
    code = 'def f(a: int) -> bool:\n    return a < 2 and "&" != "<b>"\n'
    project(tmp_path, monkeypatch, {"a.py": code})

    out = CliRunner().invoke(app, ["scan"]).stdout

    assert code.rstrip() in out


def test_only_a_closing_tag_of_our_own_blocks_is_neutralised():
    text = 'html = "</div>"\nend = "</file>"'
    assert tags.escape_xml_value(text) == 'html = "</div>"\nend = "&lt;/file>"'


def test_large_scan_without_a_terminal_says_how_to_continue(tmp_path, monkeypatch):
    project(tmp_path, monkeypatch, {f"m{i}.py": "x = 1\n" for i in range(120)})

    result = CliRunner().invoke(app, ["scan"])

    assert result.exit_code == 1
    assert "-y" in result.stderr


def test_digest_is_numbered_in_source_order_with_docstrings(tmp_path, monkeypatch):
    code = (
        "def zeta(x) -> int:\n"
        '    """Last letter, first in the file."""\n'
        "    return x\n"
        "\n"
        "class Alpha:\n"
        "    def run(self):\n"
        "        pass\n"
    )
    project(tmp_path, monkeypatch, {"m.py": code})

    out = CliRunner().invoke(app, ["digest"]).stdout

    assert "1: def zeta(x) -> int\n       Last letter, first in the file." in out
    assert out.index("def zeta") < out.index("class Alpha")
    assert "6:   def run(self)" in out


def test_digest_starts_with_the_module_docstring(tmp_path, monkeypatch):
    project(tmp_path, monkeypatch, {"m.py": '"""Coherence kernels.\n\nLonger text."""\n\nX = 1\n'})

    out = CliRunner().invoke(app, ["digest"]).stdout

    assert '1: """Coherence kernels."""\n5: X = 1' in out


def test_digest_covers_only_the_paths_given(tmp_path, monkeypatch):
    project(tmp_path, monkeypatch, {"src/a.py": "def a(): pass\n", "tests/t.py": "def t(): pass\n"})

    out = CliRunner().invoke(app, ["digest", "tests"]).stdout

    assert "t.py" in out
    assert "a.py" not in out


def test_paths_in_two_folders_keep_the_project_as_root(tmp_path, monkeypatch):
    project(tmp_path, monkeypatch, {"aps/a.m": "x = 1;\n", "run/b.m": "y = 2;\n"})

    out = CliRunner().invoke(app, ["scan", "aps/a.m", "run/b.m"]).stdout

    assert f'project="{tmp_path.name}"' in out
    assert '<file path="aps/a.m">' in out and '<file path="run/b.m">' in out


def test_matlab_outline_lists_functions_help_lines_and_sections():
    code = (
        "function [q, r] = det2lab(p, params)\n"
        "% detector frame to lab frame\n"
        "%% 1. Geometry\n"
        "%% continues the title\n"
        "%%%%%%%%%%%%\n"
        "%%not a section\n"
        "x = 1;\n"
        "function y = helper(x)\n"
    )

    assert outline.matlab(code).splitlines() == [
        "1: function [q, r] = det2lab(p, params)",
        "       detector frame to lab frame",
        "3: %% 1. Geometry",
        "8: function y = helper(x)",
    ]


def test_matlab_banner_sections_take_the_title_under_them():
    code = (
        "%% ==========\n"
        "%% Nominal voxel sizes\n"
        "%% ==========\n"
        "% computed from the detector distance\n"
        "x = 1;\n"
        "%% ----------\n"
        "% Interpolation\n"
        "y = 2;\n"
    )

    assert outline.matlab(code).splitlines() == [
        "1: %% Nominal voxel sizes",
        "6: %% Interpolation",
    ]


def test_digest_lists_class_attributes_and_dataclass_fields(tmp_path, monkeypatch):
    code = (
        "class Table:\n"
        "    axes = ('y-', 'x-', 'y-')\n"
        "    name: str = 'id01'\n"
        "\n"
        "    def run(self):\n"
        "        local = 1\n"
    )
    project(tmp_path, monkeypatch, {"m.py": code})

    out = CliRunner().invoke(app, ["digest"]).stdout

    assert "2:   axes = ('y-', 'x-', 'y-')" in out
    assert "3:   name: str = 'id01'" in out
    assert "local" not in out


def test_scan_shares_a_line_range_numbered(tmp_path, monkeypatch):
    project(tmp_path, monkeypatch, {"a.py": "".join(f"x{i} = {i}\n" for i in range(1, 21))})

    out = CliRunner().invoke(app, ["scan", "a.py:9-11", "a.py:20"]).stdout

    assert '<file path="a.py" lines="9-11">\n 9: x9 = 9\n10: x10 = 10\n11: x11 = 11\n</file>' in out
    assert '<file path="a.py" lines="20">\n20: x20 = 20\n</file>' in out
    assert "x1 = 1\n" not in out


def test_a_mistyped_range_says_what_the_syntax_is(tmp_path, monkeypatch):
    project(tmp_path, monkeypatch, {"a.py": "a = 1\n"})

    result = CliRunner().invoke(app, ["scan", "a.py:1:3"])

    assert result.exit_code == 2
    assert "FILE:START-END" in result.output


def test_a_range_past_the_end_is_cut_to_the_file(tmp_path, monkeypatch):
    project(tmp_path, monkeypatch, {"a.py": "a = 1\nb = 2\n"})

    out = CliRunner().invoke(app, ["scan", "a.py:2-99"]).stdout

    assert '<file path="a.py" lines="2">\n2: b = 2\n</file>' in out


def test_tree_depth_folds_deeper_folders_into_a_count(tmp_path, monkeypatch):
    project(tmp_path, monkeypatch, {"lib/a/x.m": "", "lib/b.m": "", "top.m": ""})

    out = CliRunner().invoke(app, ["tree", "-L", "1"]).stdout

    assert out.splitlines() == ["├── lib/ (2 files)", "└── top.m"]
