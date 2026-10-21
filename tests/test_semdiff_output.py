"""What `cdg semdiff` prints: numbered symbols, no class entry for method-only changes."""

from typer.testing import CliRunner

from codigest.core import semdiff
from codigest.main import app

OLD = '''\
class Box:
    """A box."""

    size = 1

    def area(self, k):
        return self.size * k

    def name(self):
        return "box"


def helper(a):
    return a
'''


def kinds(new):
    return {(c.change_type, c.symbol.name) for c in semdiff.compare(OLD, new)}


def test_class_entry_is_dropped_when_only_a_method_changed():
    new = OLD.replace("self.size * k", "self.size + k")

    assert kinds(new) == {("LOGIC_CHANGED", "Box.area")}


def test_class_entry_is_dropped_when_only_an_attribute_changed():
    assert kinds(OLD.replace("size = 1", "size = 2")) == {("MODIFIED", "Box.size")}


def test_class_entry_stays_when_the_class_itself_changed():
    new = OLD.replace("class Box:", "@final\nclass Box:")

    assert kinds(new) == {("LOGIC_CHANGED", "Box")}


def test_added_and_removed_methods_do_not_hide_the_class():
    new = OLD.replace('    def name(self):\n        return "box"\n', "")

    assert ("REMOVED", "Box.name") in kinds(new)
    assert ("LOGIC_CHANGED", "Box") not in kinds(new)


def run(tmp_path, monkeypatch, new):
    (tmp_path / ".git").mkdir()
    (tmp_path / "a.py").write_text(OLD, encoding="utf-8")
    monkeypatch.chdir(tmp_path)
    CliRunner().invoke(app, ["mark"])
    (tmp_path / "a.py").write_text(new, encoding="utf-8")
    return CliRunner().invoke(app, ["semdiff"]).stdout


def test_lines_are_numbered_like_digest_and_keep_the_signature_only_when_it_changed(
    tmp_path, monkeypatch
):
    new = OLD.replace("def area(self, k):", "def area(self, k, j=1):").replace(
        "return a", "return a + 1"
    )

    out = run(tmp_path, monkeypatch, new)

    assert "[SIGNATURE] 6: method Box.area :: (self, k) -> (self, k, j=1)" in out
    assert "[LOGIC]     13: function helper\n" in out
    assert "Implementation changed" not in out
    assert "class Box" not in out


def test_a_change_in_a_body_too_deep_for_unparse_is_still_found():
    def table(last: str) -> str:
        return "def total(x):\n    return " + " + ".join(["x"] * 600 + [last]) + "\n"

    changes = semdiff.compare(table("1"), table("2"))
    assert [(c.change_type, c.symbol.name) for c in changes] == [("LOGIC_CHANGED", "total")]
    # Formatting alone is still not a change.
    assert semdiff.compare(table("1"), table("1").replace(" + ", "+")) == []
