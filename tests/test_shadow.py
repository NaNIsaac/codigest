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
